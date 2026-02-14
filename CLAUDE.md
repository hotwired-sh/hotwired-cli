# hotwired-cli

Rust CLI that communicates with hotwired-core via Unix socket IPC (`~/.hotwired/hotwired.sock`).

## CLI Help Conventions

- ALL commands MUST have comprehensive `--help` with descriptions of every option
- When adding/removing commands or options, update the help text
- For commands that display structured data, the `--help` MUST include example output modeled after real data
- Keep help text concise but complete

## Architecture

- `src/main.rs` - CLI entry point, command definitions, routing
- `src/ipc.rs` - Unix socket client for hotwired-core communication
- `src/commands/` - command implementations (run, session, auth)
- No direct database access - everything goes through hotwired-core socket
- IPC params use camelCase (matching hotwired-core's serde conventions)

## Aliases

Commands have Unix-style short aliases: `list`/`ls`, `remove`/`rm`. Run IDs accept short prefixes (like git). Comment IDs can also be abbreviated.
