# hotwired-cli

Rust CLI that communicates with hotwired-core via Unix socket IPC (`~/.hotwired/hotwired.sock`).

## Git Rules

**NEVER push without being told to.** Pushes trigger CI releases.

**NEVER use breaking change indicators without explicit user approval:**
- Do NOT use `feat!:` or `fix!:` commit prefixes
- Do NOT add `BREAKING CHANGE:` or `BREAKING-CHANGE:` footers to commit messages
- These trigger **major version bumps** via semantic-release, which affect all users

If you believe a change is genuinely breaking, ASK the user first. Most changes are minor (`feat:`) or patch (`fix:`). When in doubt, use `feat:` for a minor bump.

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
