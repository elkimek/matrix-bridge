# matrix-bridge

E2EE Matrix bridge — CLI and MCP server for any AI coding agent.

Send and read end-to-end encrypted messages in Matrix rooms from your terminal or any MCP-compatible tool (Claude Code, Cursor, Windsurf, Cline, etc.). Zero system dependencies — just download and run.

## Why

AI coding agents (Claude Code, Cursor, Windsurf, etc.) are powerful but isolated — they can't talk to other bots or people outside their own session. Matrix is an open, federated, end-to-end encrypted chat protocol that solves this.

With matrix-bridge, your coding agent can:

- **Talk to other AI bots** — have Claude Code collaborate with an OpenClaw bot, a custom assistant, or any Matrix user in real time
- **Join encrypted group chats** — participate in team rooms where humans and bots discuss work together
- **Send and wait for replies** — ask a question to another bot or person and get the answer back in the same session
- **Bridge across providers** — an agent running on Anthropic can chat with one on OpenAI, or with a human on Element, all through the same Matrix room

**Real-world example:** We use this to connect [Claude Code](https://claude.ai/code) with [Žofka](https://github.com/elkimek), an OpenClaw bot running on Sonnet. They share an encrypted Matrix room where they collaborate on code, debug issues together, and relay information to the human operator — all with full E2EE.

## Features

- **End-to-end encryption** — automatic encrypt/decrypt using vodozemac (pure Rust Olm)
- **CLI** — `matrix-bridge send`, `read`, `rooms`, `send-wait`, `setup`, `config`
- **MCP server** — 5 tools exposed over stdio for any MCP client
- **Agent-agnostic** — no hardcoded bot names or provider assumptions
- **Static binaries** — no Python, no venv, no libolm
- **Cross-platform** — Linux (x86/ARM), macOS (Intel/Apple Silicon). Windows support planned.
- **TOFU trust** — Trust On First Use device verification, suitable for bot-to-bot communication

## Install

Currently from source only (GitHub Releases and crates.io coming after broader testing):

```bash
git clone https://github.com/elkimek/matrix-bridge.git
cd matrix-bridge
cargo build --release
# Binaries at target/release/matrix-bridge and target/release/matrix-bridge-mcp
```

## Quick start

### 1. Create a Matrix account

You need a Matrix account for the bridge. Create one at [element.io](https://app.element.io) or any Matrix homeserver, then note your user ID (e.g., `@mybot:matrix.org`) and password.

### 2. Setup

```bash
matrix-bridge setup
# Enter your Matrix user ID and password when prompted
# This creates encryption keys and saves your session
```

### 3. Configure a default room

```bash
# Use the config command to avoid shell escaping issues with !
matrix-bridge config default_room "!yourRoomId:matrix.org"
```

> **Tip:** Run `matrix-bridge rooms` to see your joined rooms and copy the room ID from there.

### 4. Send and read

```bash
# Send a message
matrix-bridge send "Hello from the bridge!"

# Read recent messages
matrix-bridge read --limit 10

# Send and wait for a reply (30s timeout)
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
  --room <id>                        Room ID (overrides default)
  --mention <@user:server>           @mention a user
  --no-mention                       Suppress default mention
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

### Claude Code

Add to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "matrix": {
      "command": "/path/to/matrix-bridge-mcp"
    }
  }
}
```

### Cursor

Add to `.cursor/mcp.json`:

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

Config lives at `~/.matrix-bridge/config.json` (created by `matrix-bridge setup`):

```json
{
  "homeserver": "https://matrix.org",
  "user_id": "@mybot:matrix.org",
  "device_name": "matrix-bridge",
  "store_path": "/home/user/.matrix-bridge/store",
  "trust_mode": "tofu",
  "default_room": "!roomid:matrix.org",
  "default_mention": "@friend:matrix.org",
  "notify_on_mention": "mybot"
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

## Troubleshooting

**All messages show as "[encrypted — unable to decrypt]"**

This means the bridge doesn't have the Megolm session keys for those messages. This happens when:
- You're reading messages sent before the bridge device was created — these can never be decrypted
- You need to run `matrix-bridge setup` to create a fresh device with proper key exchange

Messages sent *after* setup will decrypt normally.

**"crypto store doesn't match" error during setup**

Delete the old crypto store and run setup again:
```bash
rm ~/.matrix-bridge/store/matrix-sdk-*.sqlite3 ~/.matrix-bridge/store/credentials.json
matrix-bridge setup
```

**Shell escaping issues with room IDs**

Room IDs start with `!` which bash interprets as history expansion. Use double quotes:
```bash
matrix-bridge config default_room "!roomid:matrix.org"
```

Or edit `~/.matrix-bridge/config.json` directly.

## Building

Requires Rust 1.80+ (for dependency compatibility).

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
