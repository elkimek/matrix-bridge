# matrix-bridge

E2EE Matrix bridge — CLI and MCP server for any AI coding agent.

Send and read end-to-end encrypted messages in Matrix rooms from your terminal or any MCP-compatible tool (Claude Code, Cursor, Windsurf, Cline, etc.). Single static binary, zero system dependencies.

## Features

- **End-to-end encryption** — automatic encrypt/decrypt using vodozemac (pure Rust Olm)
- **CLI** — `matrix-bridge send`, `read`, `rooms`, `send-wait`, `setup`, `config`
- **MCP server** — 5 tools exposed over stdio for any MCP client
- **Agent-agnostic** — no hardcoded bot names or provider assumptions
- **Single binary** — no Python, no venv, no libolm. Just download and run.
- **Cross-platform** — Linux (x86/ARM), macOS (Intel/Apple Silicon). Windows support planned.
- **TOFU trust** — Trust On First Use device verification, suitable for bot-to-bot communication

## Install

### From GitHub Releases

Download the latest binary for your platform from [Releases](https://github.com/elkimek/matrix-bridge/releases).

### From crates.io

```bash
cargo install matrix-bridge
```

### From source

```bash
git clone https://github.com/elkimek/matrix-bridge.git
cd matrix-bridge
cargo build --release
# Binaries at target/release/matrix-bridge and target/release/matrix-bridge-mcp
```

## Quick start

```bash
# 1. Interactive setup — login and create encryption keys
matrix-bridge setup

# 2. Set a default room
matrix-bridge config default_room '!roomid:matrix.org'

# 3. Send a message
matrix-bridge send "Hello from the bridge!"

# 4. Read recent messages
matrix-bridge read --limit 20

# 5. Send and wait for a reply
matrix-bridge send-wait "ping" --timeout 30
```

## CLI reference

```
matrix-bridge setup                  Interactive login + key setup
matrix-bridge send <msg>             Send a message
  --room <id>                        Room ID (overrides default)
  --mention <@user:server>           @mention a user
  --no-mention                       Suppress default mention
matrix-bridge read                   Read recent messages
  --room <id>                        Room ID (overrides default)
  --limit <n>                        Number of messages (1-100, default 10)
matrix-bridge rooms                  List joined rooms
matrix-bridge send-wait <msg>        Send and wait for reply
  --timeout <secs>                   Timeout (1-300, default 30)
matrix-bridge config                 View all config
matrix-bridge config <key>           View one config key
matrix-bridge config <key> <value>   Set a config key
matrix-bridge mcp-server             Start MCP server on stdio
```

All commands support `--json` for machine-readable output.

## MCP server

The MCP server exposes 5 tools over stdin/stdout:

| Tool | Description |
|------|-------------|
| `send_message` | Send a message to a room (auto-encrypted) |
| `send_and_wait` | Send and wait for a reply with timeout |
| `read_messages` | Read recent messages (auto-decrypted) |
| `list_rooms` | List joined rooms |
| `join_room` | Join a room by ID or alias |

### Claude Code / settings.json

```json
{
  "mcpServers": {
    "matrix": {
      "command": "/path/to/matrix-bridge-mcp"
    }
  }
}
```

### Cursor / .cursor/mcp.json

```json
{
  "mcpServers": {
    "matrix": {
      "command": "/path/to/matrix-bridge-mcp"
    }
  }
}
```

Works with any MCP client that supports stdio transport.

## Configuration

Config lives at `~/.matrix-bridge/config.json`:

```json
{
  "homeserver": "https://matrix.org",
  "user_id": "@bot:matrix.org",
  "device_name": "matrix-bridge",
  "store_path": "/home/user/.matrix-bridge/store",
  "trust_mode": "tofu",
  "default_room": "!roomid:matrix.org",
  "default_mention": "@user:matrix.org",
  "notify_on_mention": "bot"
}
```

| Field | Description |
|-------|-------------|
| `homeserver` | Matrix homeserver URL |
| `user_id` | Matrix user ID |
| `device_name` | Device display name (default: "matrix-bridge") |
| `store_path` | Path for encryption keys and state |
| `trust_mode` | Device trust: `tofu`, `all`, or `explicit` |
| `default_room` | Default room for CLI commands |
| `default_mention` | Default @mention for send commands |
| `notify_on_mention` | Pattern for MCP mention notifications (defaults to local part of user_id) |

### Trust modes

- **tofu** (default) — Trust On First Use. Auto-verify new devices. Best for bots.
- **all** — Trust all devices unconditionally.
- **explicit** — Manual verification only. Most secure, requires out-of-band verification.

## Migration from the Python version

If you were using [matrix-e2ee-bridge](https://github.com/elkimek/matrix-e2ee-bridge) (Python):

1. Install the Rust binary
2. Your `config.json` works as-is — same path, same format
3. Run `matrix-bridge setup` to create a new device (crypto store formats are incompatible between Python/nio and Rust/vodozemac)
4. Update your MCP config command path from the Python venv to the Rust binary
5. Other room members will see the new device — TOFU will auto-trust it

## Building

Requires Rust 1.75+ (for async trait support).

```bash
cargo build --release
```

Feature flags:
- `cli` (default) — CLI binary with clap
- `mcp` (default) — MCP server with rmcp

Build CLI-only (no MCP):
```bash
cargo build --release --no-default-features --features cli
```

## License

GPL-3.0-or-later
