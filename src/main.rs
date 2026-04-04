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

    #[tool(description = "show git blame for a file, who last mofidied each line")]
    async fn blame(
        &self,
        Parameters(BlameRequest { path }): Parameters<BlameRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let repo_path = self.repo_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            let repo = git2::Repository::open(&repo_path)
                .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
            let blame = repo
                .blame_file(std::path::Path::new(&path), None)
                .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))?;
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

            Ok::<_, rmcp::ErrorData>(output)
        })
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(e.to_string(), None))??;

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
