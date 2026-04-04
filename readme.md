# git-forensics

MCP server in Rust that exposes git repo analysis tools over stdio.

https://github.com/user-attachments/assets/7f93067d-49da-4e82-a5e0-69837626e38f

## Tools

- **blame** — who last modified each line of a file
- **history** — commit log for a specific file
- **hotspots** — files that change most often (churn analysis)

## Setup

```bash
cargo build --release
```

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "git-forensics": {
      "command": "/path/to/target/release/mcp-test",
      "args": ["/path/to/some/git/repo"]
    }
  }
}
```

## Stack

rmcp, git2, tokio, serde, schemars, tracing, anyhow
