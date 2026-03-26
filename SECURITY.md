# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in matrix-bridge, please report it responsibly:

1. **Do not** open a public GitHub issue
2. Email **security@getbased.health** with a description of the vulnerability
3. Include steps to reproduce if possible

You should receive a response within 48 hours.

## Scope

Security issues we care about:

- **Encryption key leakage** — access tokens, Megolm session keys, or device keys exposed in logs, error messages, or temp files
- **Credential exposure** — config files or store directories with overly permissive permissions
- **E2EE bypass** — messages sent or stored unencrypted when encryption is expected
- **Remote code execution** — via malicious Matrix events, room names, or user inputs
- **MCP transport injection** — malicious input via stdio that escapes the tool boundary

## Security Design

### Credentials

- Access tokens and encryption keys are stored in `~/.matrix-bridge/store/` with `0700` directory permissions
- Config file at `~/.matrix-bridge/config.json` uses `0600` permissions
- Credential writes use atomic temp-file + rename to avoid TOCTOU races
- Access tokens are never logged — all log output goes to stderr

### Encryption

- E2EE via [vodozemac](https://github.com/matrix-org/vodozemac) (pure Rust Olm/Megolm implementation by the Matrix.org Foundation)
- No dependency on libolm (C library) — eliminates an entire class of memory safety issues
- TOFU (Trust On First Use) device verification by default
- SQLite-backed crypto store via matrix-sdk-sqlite

### Dependencies

- Built on [matrix-sdk](https://github.com/matrix-org/matrix-rust-sdk), the official Rust SDK maintained by the Matrix.org Foundation
- MCP server via [rmcp](https://github.com/modelcontextprotocol/rust-sdk), the official Rust MCP SDK

## Supported Versions

| Version | Supported |
|---------|-----------|
| Latest main | Yes |
| Older commits | No |

This project is pre-1.0. Security fixes are applied to the main branch only.
