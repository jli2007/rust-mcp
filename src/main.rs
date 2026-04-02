use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::*;
use rmcp::{tool_handler, tool_router, ServerHandler};
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;

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
