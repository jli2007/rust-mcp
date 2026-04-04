use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;

#[derive(serde::Deserialize, schemars::JsonSchema)]
struct BlameRequest {
    path: String,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
struct FileHistoryRequest {
    path: String,
    max_commits: Option<usize>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
struct HotspotRequest {
    top_n: Option<usize>,
}

// error wrapping
// no more .map_err() for each line
struct ToolError(rmcp::ErrorData);

impl From<git2::Error> for ToolError {
    fn from(e: git2::Error) -> Self {
        ToolError(rmcp::ErrorData::internal_error(e.to_string(), None))
    }
}

#[derive(Clone)]
struct GitForensicsServer {
    repo_path: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl GitForensicsServer {
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "show git blame for a file, who last modified each line")]
    async fn blame(
        &self,
        Parameters(BlameRequest { path }): Parameters<BlameRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let repo_path = self.repo_path.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
            let repo = git2::Repository::open(&repo_path)?;
            let blame = repo.blame_file(std::path::Path::new(&path), None)?;
            let mut output = String::new();
            for (_i, hunk) in blame.iter().enumerate() {
                let sig = hunk.final_signature();
                let author = sig.name().unwrap_or("unknown");
                let line = hunk.final_start_line();
                let lines = hunk.lines_in_hunk();
                output.push_str(&format!(
                    "L{}-{}: {} ({})\n",
                    line,
                    line + lines - 1,
                    author,
                    &hunk.final_commit_id().to_string()[..8],
                ));
            }

            Ok(output)
        })
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
        .map_err(|e| e.0)?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "show commit history for a specific file")]
    async fn history(
        &self,
        Parameters(FileHistoryRequest { path, max_commits }): Parameters<FileHistoryRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let repo_path = self.repo_path.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
            let repo = git2::Repository::open(&repo_path)?;
            let mut revwalk = repo.revwalk()?;
            revwalk.push_head()?;
            revwalk.set_sorting(git2::Sort::TIME)?;
            let mut output = String::new();
            let limit = max_commits.unwrap_or(20);
            let mut count = 0;

            for oid in revwalk {
                let oid = oid?;
                let commit = repo.find_commit(oid)?;

                let tree = commit.tree()?;
                let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

                let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

                let touched = diff
                    .deltas()
                    .any(|d| d.new_file().path() == Some(std::path::Path::new(&path)));

                if !touched {
                    continue;
                }

                let id = &commit.id().to_string()[..8];
                let sig = commit.author();
                let author = sig.name().unwrap_or("unknown");
                let message = commit.summary();
                let summary = message.unwrap_or("");
                let time = commit.time().seconds();
                output.push_str(&format!("{} {} — {} ({})\n", id, time, author, summary));
                count += 1;
                if count >= limit {
                    break;
                }
            }
            Ok(output)
        })
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
        .map_err(|e| e.0)?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }

    #[tool(description = "find files that change most often")]
    async fn hotspots(
        &self,
        Parameters(HotspotRequest { top_n }): Parameters<HotspotRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let repo_path = self.repo_path.clone();
        let result = tokio::task::spawn_blocking(move || -> Result<String, ToolError> {
            let repo = git2::Repository::open(&repo_path)?;
            let mut revwalk = repo.revwalk()?;
            revwalk.push_head()?;
            revwalk.set_sorting(git2::Sort::TIME)?;
            let mut output = String::new();
            let mut counts = std::collections::HashMap::new();

            for oid in revwalk {
                let oid = oid?;
                let commit = repo.find_commit(oid)?;

                let tree = commit.tree()?;
                let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

                let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

                for delta in diff.deltas() {
                    if let Some(p) = delta.new_file().path() {
                        let key = p.to_string_lossy().to_string();
                        *counts.entry(key).or_insert(0) += 1;
                    }
                }
            }
            let mut sorted: Vec<_> = counts.into_iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(&a.1));
            let limit = top_n.unwrap_or(10);
            for (file, count) in sorted.into_iter().take(limit) {
                output.push_str(&format!("{} — {} commits\n", file, count));
            }
            Ok(output)
        })
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?
        .map_err(|e| e.0)?;

        Ok(CallToolResult::success(vec![Content::text(result)]))
    }
}

// for the mcp
#[tool_handler]
impl ServerHandler for GitForensicsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "git-forensics",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions("Analyze git repositories for blame, history, and hotspots.")
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let repo_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let server = GitForensicsServer::new(repo_path);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
