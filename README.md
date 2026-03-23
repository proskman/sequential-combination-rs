# 🦀 Sequential Combination MCP — Rust Port

**Zero-Dependency, High-Performance Rust port of the Sequential Combination MCP Server.**

Designed to eliminate restart loops, Python conflicts, and pip installation issues. Delivers 10-50x performance improvements over the Python version.

## 🔥 Why Rust vs Python?

| Feature | Python (Original) | Rust (This Port) |
|---|---|---|
| Startup Time | 5-15s | < 500ms |
| Memory | ~2 GB | ~50 MB |
| Deployment | `venv` + `pip install` | **Single binary** |
| Restart Loop | ❌ stdout pollution | ✅ All logs → stderr |

## 🚀 No Installation Required

GitHub Actions automatically compiles binaries for all platforms on every release.

**Download your binary from [Releases](../../releases)** — no Rust, no Python, no pip needed.

## ⚙️ VSCode / Kilocode Configuration

```json
{
  "mcpServers": {
    "sequential-combination-rs": {
      "command": "C:/path/to/sequential-combination-rs.exe",
      "env": {
        "MCP_BASE_DIR": "C:/path/to/sequential-combination-rs",
        "RUST_LOG": "info"
      }
    }
  }
}
```

## 🧰 Available Tools

| Tool | Description |
|---|---|
| `ping` | Health check |
| `list_stages` | List all cognitive stages |
| `suggest_combo` | Semantic skill suggestion |
| `get_expert_dna` | Condensed expert DNA extraction |
| `load_combo_content` | Full SKILL.md content loader |

## 📄 License
MIT
