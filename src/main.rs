use rmcp::handler::server::tool::ToolRouter;
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;

#[derive(Clone)]
struct GitForensicsServer {
    repo_path: PathBuf,
    tool_router: ToolRouter<Self>,
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
    let server = GitForensicsServer {
        repo_path,
        tool_router: GitForensicsServer::tool_router(),
    };
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
