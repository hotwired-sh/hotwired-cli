# CLI Inbox/Outbox Implementation Plan

## Overview

Replace MCP-based agent interaction with CLI commands. The CLI talks to hotwired-core via Unix socket (already exists) and prints output directly to the agent's terminal.

**Key Insight**: The CLI runs IN the Zellij terminal. Whatever it prints, the agent sees. No special "signaling" mechanism needed.

## Why This Matters

### Current MCP Problem

```
Agent calls MCP tool: hotwired_hotwire(playbook="plan-build", intent="...")
    ↓
MCP returns: { runId: "...", protocol: "<5KB of instructions>" }
    ↓
ENTIRE RESPONSE GOES INTO AGENT CONTEXT (context bloat!)
```

### CLI Solution

```
Agent runs: hotwired hotwire --playbook plan-build --intent "..."
    ↓
CLI prints to terminal:
  "Run started: abc123
   Your role: strategist

   [Protocol instructions here - agent reads but doesn't consume context]"
    ↓
Agent sees output, context stays clean
```

## Architecture

```
┌─────────────────┐     Unix Socket      ┌─────────────────┐
│  hotwired-cli   │ ──────────────────▶  │  hotwired-core  │
│  (Rust binary)  │ ◀──────────────────  │  (handlers)     │
└─────────────────┘                      └─────────────────┘
        │                                         │
        │ prints to                               │ broadcasts
        ▼ terminal                                ▼ WebSocket
┌─────────────────┐                      ┌─────────────────┐
│  Agent Terminal │                      │  Hotwired App   │
│  (Zellij pane)  │                      │  (React UI)     │
└─────────────────┘                      └─────────────────┘
```

## What Already Exists

### hotwired-cli (~/Code/hotwired-sh/hotwired-cli)

- IPC client talking to socket (`src/ipc.rs`)
- Auth token handling
- Commands: `run list/show/rm`, `session list/show/rm`, `auth status`

### hotwired-core socket handlers

Already implemented in `packages/hotwired-core/src/socket/mod.rs`:
- `hotwire` - Start a run (line 719)
- `pair` - Join a run (line 727)
- `request_pair` - Request another agent (line 735)
- `handoff` - Send work to another agent (line 427)
- `send_message` - Log a message (line 460)
- `task_complete` - Mark task done (line 485)
- `report_impediment` - Report blocker (line 402)
- `request_input` - Ask human (line 438)
- `get_run_status` - Check run state (line 316)

## Implementation Plan

### Phase 0: Session Validation (CRITICAL)

**Every command except `hotwire` and `pair` MUST validate session state first.**

The CLI runs in a Zellij terminal. If that terminal isn't attached to a run, most commands are meaningless. The CLI must:

1. Read `$ZELLIJ_SESSION_NAME` from environment
2. Query hotwired-core for session state
3. If not attached to a run, print error and exit

```rust
// commands/validate.rs

pub struct SessionState {
    pub session_name: String,
    pub run_id: String,
    pub role_id: String,
    pub run_status: String,  // active, paused, completed
}

pub enum ValidationError {
    NoZellijSession,      // Not running in Zellij
    SessionNotRegistered, // Zellij session unknown to hotwired-core
    NotAttachedToRun,     // Session exists but not paired to a run
    RunNotActive,         // Run exists but is completed/cancelled
}

/// Call this FIRST in every command (except hotwire/pair)
pub async fn validate_session(client: &HotwiredClient) -> Result<SessionState, ValidationError> {
    // 1. Get Zellij session name
    let session_name = std::env::var("ZELLIJ_SESSION_NAME")
        .map_err(|_| ValidationError::NoZellijSession)?;

    // 2. Query hotwired-core for this session
    let resp = client.request("get_session_state", json!({
        "zellij_session": session_name,
    })).await
        .map_err(|_| ValidationError::SessionNotRegistered)?;

    // 3. Check if attached to a run
    let run_id = resp.data["attached_run_id"].as_str()
        .ok_or(ValidationError::NotAttachedToRun)?;

    // 4. Check run is active
    let run_status = resp.data["run_status"].as_str().unwrap_or("unknown");
    if run_status != "active" && run_status != "paused" {
        return Err(ValidationError::RunNotActive);
    }

    Ok(SessionState {
        session_name,
        run_id: run_id.to_string(),
        role_id: resp.data["role_id"].as_str().unwrap_or("unknown").to_string(),
        run_status: run_status.to_string(),
    })
}

/// Print user-friendly error message
pub fn print_validation_error(err: ValidationError) {
    match err {
        ValidationError::NoZellijSession => {
            eprintln!("ERROR: Not running in a Zellij session.");
            eprintln!("The hotwired CLI must be run from within a Hotwired-managed terminal.");
        }
        ValidationError::SessionNotRegistered => {
            eprintln!("ERROR: This terminal is not registered with Hotwired.");
            eprintln!("Run `hotwired pair <RUN_ID>` to join an existing run.");
        }
        ValidationError::NotAttachedToRun => {
            eprintln!("ERROR: This terminal is not attached to any run.");
            eprintln!("Run `hotwired pair <RUN_ID>` to join a run, or");
            eprintln!("Run `hotwired hotwire --intent \"...\"` to start a new run.");
        }
        ValidationError::RunNotActive => {
            eprintln!("ERROR: The attached run is no longer active.");
            eprintln!("Run `hotwired pair <RUN_ID>` to join a different run.");
        }
    }
    std::process::exit(1);
}
```

**Usage in commands:**

```rust
// commands/send.rs
pub async fn send(client: &HotwiredClient, to: &str, message: &str) {
    // FIRST: validate session
    let state = match validate_session(client).await {
        Ok(s) => s,
        Err(e) => {
            print_validation_error(e);
            return;
        }
    };

    // Now we have state.run_id, state.role_id, etc.
    let response = client.request("handoff", json!({
        "run_id": state.run_id,
        "to": to,
        "summary": truncate(message, 50),
        "details": message,
        "source": state.role_id,
    })).await;
    // ...
}
```

**Backend requirement:** Need a `get_session_state` socket handler that returns:
```json
{
  "zellij_session": "abc123",
  "attached_run_id": "run_xyz",  // null if not attached
  "role_id": "worker-1",
  "run_status": "active"
}
```

### Phase 1: Core CLI Commands

Add to `hotwired-cli/src/commands/`:

#### 1.1 `hotwired hotwire` (Priority: HIGH)

Start a new workflow run.

```bash
# Usage
hotwired hotwire [OPTIONS]

# Options
--playbook <ID>     Playbook to use (default: plan-build)
--intent <TEXT>     What you want to accomplish
--project <PATH>    Project path (default: $PWD)

# Example
hotwired hotwire --playbook architect-team --intent "Build user auth feature"
```

**Implementation**:
```rust
// commands/hotwire.rs
pub async fn hotwire(client: &HotwiredClient, args: HotwireArgs) {
    let session_name = std::env::var("ZELLIJ_SESSION_NAME")
        .unwrap_or_else(|_| "unknown".to_string());

    let response = client.request("hotwire", json!({
        "zellij_session": session_name,
        "project_path": args.project.unwrap_or_else(|| std::env::current_dir()),
        "suggested_playbook": args.playbook,
        "intent": args.intent,
    })).await;

    match response {
        Ok(resp) if resp.data["status"] == "started" => {
            println!("Run started: {}", resp.data["runId"]);
            println!("Role: {}", resp.data["role"]);
            println!("\n{}", resp.data["protocol"]);  // Agent reads this
        }
        Ok(resp) if resp.data["status"] == "needs_confirmation" => {
            println!("Awaiting confirmation in Hotwired app...");
            println!("Run ID: {}", resp.data["pendingRunId"]);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

#### 1.2 `hotwired pair` (Priority: HIGH)

Join an existing run.

```bash
# Usage
hotwired pair <RUN_ID> [OPTIONS]

# Options
--role <ROLE_ID>    Role to take (e.g., worker-1, builder)

# Example
hotwired pair abc123 --role worker-1
```

**Implementation**:
```rust
// commands/pair.rs
pub async fn pair(client: &HotwiredClient, run_id: &str, role: Option<&str>) {
    let session_name = std::env::var("ZELLIJ_SESSION_NAME")
        .unwrap_or_else(|_| "unknown".to_string());

    let response = client.request("pair", json!({
        "zellij_session": session_name,
        "run_id": run_id,
        "role_id": role,
    })).await;

    match response {
        Ok(resp) => {
            println!("Joined run: {}", run_id);
            println!("Role: {}", resp.data["role"]);
            println!("\n{}", resp.data["protocol"]);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

#### 1.3 `hotwired send` (Priority: HIGH)

Send a message/handoff to another participant.

```bash
# Usage
hotwired send --to <TARGET> <MESSAGE>

# Options
--to <TARGET>       Recipient: "orchestrator", "implementer", "human", or role ID

# Examples
hotwired send --to orchestrator "Task 1.1 complete. Tests passing."
hotwired send --to human "Need clarification on auth approach"
```

**Implementation**:
```rust
// commands/send.rs
pub async fn send(client: &HotwiredClient, to: &str, message: &str) {
    // FIRST: validate session state (see Phase 0)
    let state = match validate_session(client).await {
        Ok(s) => s,
        Err(e) => return print_validation_error(e),
    };

    let response = client.request("handoff", json!({
        "run_id": state.run_id,
        "to": to,
        "summary": truncate(message, 50),
        "details": message,
        "source": state.role_id,
    })).await;

    match response {
        Ok(_) => println!("Sent to {}", to),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

#### 1.4 `hotwired inbox` (Priority: MEDIUM)

Check for messages (polls conversation events).

```bash
# Usage
hotwired inbox [OPTIONS]

# Options
--watch             Continuously poll for new messages
--since <EVENT_ID>  Only show messages after this ID

# Example
hotwired inbox --watch
```

**Implementation**:
```rust
// commands/inbox.rs
pub async fn inbox(client: &HotwiredClient, watch: bool, since: Option<i64>) {
    // FIRST: validate session state (see Phase 0)
    let state = match validate_session(client).await {
        Ok(s) => s,
        Err(e) => return print_validation_error(e),
    };

    let response = client.request("get_conversation_events", json!({
        "run_id": state.run_id,
        "since_sequence": since,
        "limit": 10,
    })).await;

    match response {
        Ok(resp) => {
            let events = resp.data["events"].as_array().unwrap_or(&vec![]);
            if events.is_empty() {
                println!("No new messages.");
            } else {
                for event in events {
                    println!("[{}] {}: {}",
                        event["source"],
                        event["eventType"],
                        event["content"]);
                }
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

#### 1.5 `hotwired complete` (Priority: MEDIUM)

Mark the task as complete.

```bash
# Usage
hotwired complete [OPTIONS]

# Options
--outcome <TEXT>    Outcome description

# Example
hotwired complete --outcome "All tests passing, feature deployed"
```

#### 1.6 `hotwired impediment` (Priority: MEDIUM)

Report a blocker.

```bash
# Usage
hotwired impediment <DESCRIPTION> [OPTIONS]

# Options
--type <TYPE>       technical, access, clarification, decision (default: technical)
--suggestion <TEXT> Suggested resolution

# Example
hotwired impediment "Cannot push to remote - need CI to pass" --suggestion "Human to push"
```

#### 1.7 `hotwired status` (Priority: LOW)

Check current run status.

```bash
# Usage
hotwired status

# Output
Run: abc123
Status: active
Phase: executing
My Role: strategist
Connected Agents:
  - strategist (me) - active
  - builder - awaiting_response
```

### Phase 2: Helper Functions

Add to `hotwired-cli/src/commands/mod.rs`:

```rust
// Session validation is now the primary helper (see Phase 0)
// These are convenience wrappers that call validate_session internally

/// Truncate message for summary field
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Format timestamp for display
fn format_timestamp(ts: i64) -> String {
    // Convert unix timestamp to human readable
}
```

**Note:** The old `get_attached_run_id` and `get_my_role` functions are replaced by `validate_session()` which returns all state in one call.

### Phase 3: Update main.rs

```rust
// hotwired-cli/src/main.rs

#[derive(Subcommand)]
enum Commands {
    // Existing...
    Run { ... },
    Session { ... },
    Auth { ... },

    // New workflow commands
    /// Start a new workflow run
    Hotwire {
        #[arg(long)]
        playbook: Option<String>,
        #[arg(long)]
        intent: Option<String>,
        #[arg(long)]
        project: Option<PathBuf>,
    },

    /// Join an existing run
    Pair {
        run_id: String,
        #[arg(long)]
        role: Option<String>,
    },

    /// Send message to another participant
    Send {
        #[arg(long)]
        to: String,
        message: String,
    },

    /// Check for incoming messages
    Inbox {
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        since: Option<i64>,
    },

    /// Mark task complete
    Complete {
        #[arg(long)]
        outcome: Option<String>,
    },

    /// Report a blocker
    Impediment {
        description: String,
        #[arg(long, default_value = "technical")]
        r#type: String,
        #[arg(long)]
        suggestion: Option<String>,
    },

    /// Check run status
    Status,
}
```

### Phase 4: Backend Adjustments

The socket handlers mostly exist. Required additions:

1. **`get_session_state` handler (REQUIRED for Phase 0)**

   New socket handler that returns session and run state:
   ```rust
   // socket/mod.rs - add new handler
   "get_session_state" => {
       let zellij_session = payload["zellij_session"].as_str()?;

       // Look up session by Zellij name
       let session = handlers::terminal::get_session_by_zellij_name(zellij_session)?;

       // If session has attached run, get run status
       let (run_id, role_id, run_status) = if let Some(rid) = session.attached_run_id {
           let run = handlers::run::get_run(&rid)?;
           (Some(rid), session.role_id, run.status)
       } else {
           (None, None, None)
       };

       Ok(json!({
           "zellij_session": zellij_session,
           "attached_run_id": run_id,
           "role_id": role_id,
           "run_status": run_status,
       }))
   }
   ```

2. **`hotwire` response** - Currently returns full protocol. Keep this (CLI will print it).

3. **`pair` handler** - May need to add if not complete. Check `terminal.rs`.

4. **Session-to-Zellij mapping** - Ensure `terminal.rs` stores `zellij_session_name` when sessions are created, so we can look them up later.

### Phase 5: Update Playbook Prompts

After CLI works, update playbook prompts to use CLI instead of MCP:

```markdown
## Communication

Use the `hotwired` CLI:

- `hotwired send --to orchestrator "Task complete"` - Send message
- `hotwired inbox` - Check for messages
- `hotwired complete` - Mark task done
- `hotwired impediment "description"` - Report blocker

Example handoff:
\```bash
hotwired send --to implementer "Begin Task 1.1: Add OAuth dependencies to Cargo.toml. Acceptance: cargo check passes."
\```
```

## File Changes Summary

### hotwired-cli (new files)

```
src/commands/
├── mod.rs          # Update exports
├── validate.rs     # NEW - Session validation (Phase 0, implement FIRST)
├── hotwire.rs      # NEW
├── pair.rs         # NEW
├── send.rs         # NEW
├── inbox.rs        # NEW
├── complete.rs     # NEW
├── impediment.rs   # NEW
└── status.rs       # NEW
```

### hotwired-cli (modified)

```
src/main.rs         # Add new subcommands
```

### hotwired-core (possibly)

```
src/handlers/terminal.rs    # Minor tweaks if needed
src/socket/mod.rs           # Add any missing routes
```

### playbooks (later)

```
playbooks/*/protocol.md     # Update to use CLI
playbooks/*/architect.md    # Update examples
playbooks/*/worker.md       # Update examples
```

## Testing Plan

1. **Unit tests** for each CLI command
2. **Integration test**: Full workflow with CLI
   - Start run with `hotwired hotwire`
   - Check status with `hotwired status`
   - Send message with `hotwired send`
   - Complete with `hotwired complete`
3. **Manual test**: Run actual playbook using CLI instead of MCP

## Migration Path

1. **Implement `get_session_state` socket handler** (Phase 4 prerequisite)
2. **Implement `validate.rs`** (Phase 0 - blocks everything else)
3. Implement CLI commands (Phase 1-3)
4. Test CLI manually alongside MCP
5. Update one playbook (architect-team) to use CLI
6. Test with real agents
7. Update remaining playbooks
8. Deprecate MCP tools (keep for backwards compat)

## Open Questions

1. **Inbox polling vs push**: Should `inbox --watch` poll, or should we have a different mechanism?
2. **Message formatting**: How verbose should CLI output be? Agent-readable vs minimal?
3. **Error handling**: What should CLI print on errors? Should it suggest fixes?

---

## Appendix A: Existing CLI Patterns

This appendix documents the existing hotwired-cli patterns that new commands MUST follow.

### A.1 Project Structure

```
hotwired-cli/
├── Cargo.toml
└── src/
    ├── main.rs           # Clap CLI definition, command routing
    ├── ipc.rs            # HotwiredClient, IPC protocol
    └── commands/
        ├── mod.rs        # Exports, shared helpers (handle_error, format_timestamp)
        ├── auth.rs       # `hotwired auth status`
        ├── run.rs        # `hotwired run list/show/rm`
        └── session.rs    # `hotwired session list/show/rm`
```

### A.2 IPC Client API

```rust
// ipc.rs - The client for talking to hotwired-core

pub struct HotwiredClient {
    socket_path: String,      // ~/.hotwired/hotwired.sock
    auth_token: Option<String>, // ~/.hotwired/auth_token
}

impl HotwiredClient {
    /// Create client with optional custom socket path
    pub fn new(socket_path: Option<String>) -> Self;

    /// Send request to hotwired-core
    /// Returns SocketResponse with success/data/error fields
    pub async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<SocketResponse, IpcError>;

    /// Health check (calls "ping" method)
    pub async fn health_check(&self) -> Result<SocketResponse, IpcError>;
}

#[derive(Debug, Deserialize)]
pub struct SocketResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("Hotwired backend is not running (socket not found at {0})")]
    NotConnected(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}
```

### A.3 Socket Protocol

JSON-RPC-like protocol over Unix socket (`~/.hotwired/hotwired.sock`):

**Request:**
```json
{
  "id": "optional-request-id",
  "method": "method_name",
  "params": { ... },
  "token": "auth-token-from-file"
}
```

**Success Response:**
```json
{
  "id": "optional-request-id",
  "success": true,
  "data": { ... }
}
```

**Error Response:**
```json
{
  "id": "optional-request-id",
  "success": false,
  "error": "Error message"
}
```

### A.4 Command Implementation Pattern

Every command follows this pattern:

```rust
// commands/example.rs
use crate::ipc::HotwiredClient;
use super::handle_error;

pub async fn my_command(client: &HotwiredClient, arg: &str) {
    match client
        .request("method_name", serde_json::json!({"key": arg}))
        .await
    {
        Ok(response) if response.success => {
            // Handle success - extract data and print
            let value = response
                .data
                .as_ref()
                .and_then(|d| d.get("field"))
                .and_then(|v| v.as_str())
                .unwrap_or("-");
            println!("Result: {}", value);
        }
        Ok(response) => {
            // Server returned error
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "unknown error".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e), // IPC error (not connected, etc.)
    }
}
```

### A.5 main.rs Subcommand Pattern

```rust
// main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hotwired")]
struct Args {
    #[arg(long, short = 's', global = true)]
    socket_path: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Existing commands...
    Run { #[command(subcommand)] action: RunAction },
    Session { #[command(subcommand)] action: SessionAction },
    Auth { #[command(subcommand)] action: AuthAction },

    // NEW: Top-level workflow commands (no subcommand nesting)

    /// Start a new workflow run
    Hotwire {
        #[arg(long)]
        playbook: Option<String>,
        #[arg(long)]
        intent: Option<String>,
        #[arg(long)]
        project: Option<std::path::PathBuf>,
    },

    /// Join an existing run
    Pair {
        /// Run ID to join
        run_id: String,
        #[arg(long)]
        role: Option<String>,
    },

    /// Send message to another participant
    Send {
        #[arg(long)]
        to: String,
        /// Message content (use quotes for multi-word)
        message: Vec<String>,  // Collect remaining args
    },

    // ... etc
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.command {
        Some(Commands::Hotwire { playbook, intent, project }) => {
            let client = ipc::HotwiredClient::new(args.socket_path);
            commands::hotwire::run(&client, playbook, intent, project).await;
        }
        Some(Commands::Pair { run_id, role }) => {
            let client = ipc::HotwiredClient::new(args.socket_path);
            commands::pair::run(&client, &run_id, role.as_deref()).await;
        }
        Some(Commands::Send { to, message }) => {
            let client = ipc::HotwiredClient::new(args.socket_path);
            let msg = message.join(" ");
            commands::send::run(&client, &to, &msg).await;
        }
        // ... existing commands
    }
    Ok(())
}
```

### A.6 Shared Helpers (commands/mod.rs)

```rust
// commands/mod.rs
pub mod auth;
pub mod run;
pub mod session;
// NEW modules:
pub mod validate;  // Session validation (Phase 0)
pub mod hotwire;
pub mod pair;
pub mod send;
pub mod inbox;
pub mod complete;
pub mod impediment;
pub mod status;

use crate::ipc::IpcError;

/// Handle IPC errors with user-friendly messages
pub fn handle_error(e: IpcError) -> ! {
    match e {
        IpcError::NotConnected(_) => {
            eprintln!("error: not connected - is Hotwired.sh desktop app running?");
        }
        _ => {
            eprintln!("error: {}", e);
        }
    }
    std::process::exit(1);
}

/// Format ISO timestamp for display
pub fn format_timestamp(ts: &str) -> String {
    ts.replace('T', " ").trim_end_matches('Z').to_string()
}

/// Truncate string with ellipsis
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
```

---

## Appendix B: Complete validate.rs Implementation

```rust
// commands/validate.rs
//
// Session validation - MUST be called first by all commands except hotwire/pair

use crate::ipc::{HotwiredClient, IpcError};

/// Current session state from hotwired-core
#[derive(Debug)]
pub struct SessionState {
    pub zellij_session: String,
    pub run_id: String,
    pub role_id: String,
    pub run_status: String,
}

/// Validation errors with user-friendly messages
#[derive(Debug)]
pub enum ValidationError {
    /// Not running inside a Zellij terminal
    NoZellijSession,
    /// Zellij session exists but hotwired-core doesn't know about it
    SessionNotRegistered,
    /// Session exists but not attached to any run
    NotAttachedToRun,
    /// Attached run is completed/cancelled
    RunNotActive(String), // includes the status
    /// IPC error (backend not running, etc.)
    IpcError(IpcError),
}

/// Validate session state - call this FIRST in every command except hotwire/pair
pub async fn validate_session(client: &HotwiredClient) -> Result<SessionState, ValidationError> {
    // 1. Check we're in a Zellij session
    let zellij_session = std::env::var("ZELLIJ_SESSION_NAME")
        .map_err(|_| ValidationError::NoZellijSession)?;

    // 2. Query hotwired-core for session state
    let response = client
        .request(
            "get_session_state",
            serde_json::json!({"zellij_session": zellij_session}),
        )
        .await
        .map_err(ValidationError::IpcError)?;

    if !response.success {
        return Err(ValidationError::SessionNotRegistered);
    }

    let data = response.data.ok_or(ValidationError::SessionNotRegistered)?;

    // 3. Check if attached to a run
    let run_id = data
        .get("attached_run_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(ValidationError::NotAttachedToRun)?
        .to_string();

    // 4. Check run status
    let run_status = data
        .get("run_status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    if run_status != "active" && run_status != "paused" {
        return Err(ValidationError::RunNotActive(run_status));
    }

    // 5. Get role
    let role_id = data
        .get("role_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(SessionState {
        zellij_session,
        run_id,
        role_id,
        run_status,
    })
}

/// Print user-friendly error and exit
pub fn print_validation_error(err: ValidationError) -> ! {
    match err {
        ValidationError::NoZellijSession => {
            eprintln!("ERROR: Not running in a Zellij session.");
            eprintln!();
            eprintln!("The hotwired CLI must be run from within a Hotwired-managed terminal.");
            eprintln!("Start a terminal from the Hotwired app, or check $ZELLIJ_SESSION_NAME.");
        }
        ValidationError::SessionNotRegistered => {
            eprintln!("ERROR: This terminal is not registered with Hotwired.");
            eprintln!();
            eprintln!("To join an existing run:");
            eprintln!("  hotwired pair <RUN_ID>");
            eprintln!();
            eprintln!("To start a new run:");
            eprintln!("  hotwired hotwire --intent \"what you want to do\"");
        }
        ValidationError::NotAttachedToRun => {
            eprintln!("ERROR: This terminal is not attached to any workflow run.");
            eprintln!();
            eprintln!("To join an existing run:");
            eprintln!("  hotwired pair <RUN_ID>");
            eprintln!();
            eprintln!("To start a new run:");
            eprintln!("  hotwired hotwire --intent \"what you want to do\"");
        }
        ValidationError::RunNotActive(status) => {
            eprintln!("ERROR: The attached run is no longer active (status: {}).", status);
            eprintln!();
            eprintln!("To join a different run:");
            eprintln!("  hotwired pair <RUN_ID>");
        }
        ValidationError::IpcError(e) => {
            eprintln!("ERROR: {}", e);
            if matches!(e, IpcError::NotConnected(_)) {
                eprintln!();
                eprintln!("Is the Hotwired desktop app running?");
            }
        }
    }
    std::process::exit(1);
}

/// Convenience: validate and return state, or print error and exit
pub async fn require_session(client: &HotwiredClient) -> SessionState {
    match validate_session(client).await {
        Ok(state) => state,
        Err(e) => print_validation_error(e),
    }
}
```

---

## Appendix C: Backend Handler for get_session_state

Add to `hotwired-core/src/socket/mod.rs` in the match statement:

```rust
"get_session_state" => {
    let zellij_session = params
        .get("zellij_session")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CoreError::InvalidRequest("missing zellij_session".into()))?;

    // Look up session by Zellij session name
    let session = handlers::terminal::get_session_by_zellij_name(&ctx, zellij_session).await?;

    match session {
        Some(s) => {
            // Get run status if attached
            let (run_status, role_id) = if let Some(ref run_id) = s.attached_run_id {
                let run = handlers::run::get_run(&ctx, run_id).await.ok();
                (
                    run.as_ref().map(|r| r.status.as_str()).unwrap_or("unknown"),
                    s.role_id.clone(),
                )
            } else {
                ("none", None)
            };

            Ok(SocketResponse::success(
                request.id.clone(),
                serde_json::json!({
                    "zellij_session": zellij_session,
                    "session_name": s.session_name,
                    "attached_run_id": s.attached_run_id,
                    "role_id": role_id,
                    "run_status": run_status,
                }),
            ))
        }
        None => {
            // Session not found - return success but with null values
            // (let CLI handle the "not registered" case)
            Ok(SocketResponse::success(
                request.id.clone(),
                serde_json::json!({
                    "zellij_session": zellij_session,
                    "session_name": null,
                    "attached_run_id": null,
                    "role_id": null,
                    "run_status": null,
                }),
            ))
        }
    }
}
```

**Required helper in terminal.rs:**

```rust
/// Look up a session by its Zellij session name
pub async fn get_session_by_zellij_name(
    ctx: &Context,
    zellij_session: &str,
) -> Result<Option<Session>, CoreError> {
    let sessions = ctx.sessions.read().await;
    Ok(sessions
        .values()
        .find(|s| s.zellij_session_name.as_deref() == Some(zellij_session))
        .cloned())
}
```

**Prerequisite:** Session struct must store `zellij_session_name` when sessions are created/registered. Check if this field exists; if not, add it to the Session type.

---

## Appendix D: Complete Command Implementations

### D.1 hotwire.rs - Start a new workflow run

```rust
// commands/hotwire.rs
use crate::ipc::HotwiredClient;
use super::handle_error;
use std::path::PathBuf;

pub async fn run(
    client: &HotwiredClient,
    playbook: Option<String>,
    intent: Option<String>,
    project: Option<PathBuf>,
) {
    // hotwire does NOT require existing session - it creates one
    let zellij_session = std::env::var("ZELLIJ_SESSION_NAME").ok();

    if zellij_session.is_none() {
        eprintln!("WARNING: Not running in a Zellij session.");
        eprintln!("The run will start but this terminal won't be attached.");
        eprintln!();
    }

    let project_path = project
        .or_else(|| std::env::current_dir().ok())
        .map(|p| p.to_string_lossy().to_string());

    match client
        .request(
            "hotwire",
            serde_json::json!({
                "zellij_session": zellij_session,
                "project_path": project_path,
                "suggested_playbook": playbook,
                "intent": intent,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.as_ref().unwrap();
            let status = data.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");

            match status {
                "started" => {
                    let run_id = data.get("runId").and_then(|v| v.as_str()).unwrap_or("-");
                    let role = data.get("role").and_then(|v| v.as_str()).unwrap_or("-");
                    let protocol = data.get("protocol").and_then(|v| v.as_str()).unwrap_or("");

                    println!("Run started: {}", run_id);
                    println!("Your role: {}", role);
                    println!();
                    println!("{}", protocol);
                }
                "needs_confirmation" => {
                    let pending_id = data.get("pendingRunId").and_then(|v| v.as_str()).unwrap_or("-");
                    println!("Run pending confirmation: {}", pending_id);
                    println!();
                    println!("Please confirm the run in the Hotwired app.");
                    println!("Once confirmed, run: hotwired pair {}", pending_id);
                }
                _ => {
                    println!("Unexpected status: {}", status);
                    if let Some(d) = response.data {
                        println!("{}", serde_json::to_string_pretty(&d).unwrap_or_default());
                    }
                }
            }
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "unknown error".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}
```

### D.2 pair.rs - Join an existing run

```rust
// commands/pair.rs
use crate::ipc::HotwiredClient;
use super::handle_error;

pub async fn run(client: &HotwiredClient, run_id: &str, role: Option<&str>) {
    // pair does NOT require existing session - it creates the attachment
    let zellij_session = std::env::var("ZELLIJ_SESSION_NAME").ok();

    if zellij_session.is_none() {
        eprintln!("ERROR: Not running in a Zellij session.");
        eprintln!("The hotwired CLI must be run from within a Hotwired-managed terminal.");
        std::process::exit(1);
    }

    match client
        .request(
            "pair",
            serde_json::json!({
                "zellij_session": zellij_session,
                "run_id": run_id,
                "role_id": role,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.as_ref().unwrap();
            let role = data.get("role").and_then(|v| v.as_str()).unwrap_or("-");
            let protocol = data.get("protocol").and_then(|v| v.as_str()).unwrap_or("");

            println!("Joined run: {}", run_id);
            println!("Your role: {}", role);
            println!();
            println!("{}", protocol);
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "unknown error".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}
```

### D.3 send.rs - Send message to another participant

```rust
// commands/send.rs
use crate::ipc::HotwiredClient;
use super::{handle_error, truncate, validate};

pub async fn run(client: &HotwiredClient, to: &str, message: &str) {
    // Validate session first
    let state = validate::require_session(client).await;

    match client
        .request(
            "handoff",
            serde_json::json!({
                "run_id": state.run_id,
                "to": to,
                "summary": truncate(message, 50),
                "details": message,
                "source": state.role_id,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            println!("Sent to {}", to);
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "failed to send".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}
```

### D.4 inbox.rs - Check for incoming messages

```rust
// commands/inbox.rs
use crate::ipc::HotwiredClient;
use super::{handle_error, format_timestamp, validate};

pub async fn run(client: &HotwiredClient, watch: bool, since: Option<i64>) {
    // Validate session first
    let state = validate::require_session(client).await;

    if watch {
        // Continuous polling mode
        let mut last_seq = since.unwrap_or(0);
        println!("Watching for messages... (Ctrl+C to stop)");
        println!();

        loop {
            match fetch_messages(client, &state.run_id, Some(last_seq)).await {
                Ok((events, max_seq)) => {
                    for event in events {
                        print_event(&event);
                    }
                    if max_seq > last_seq {
                        last_seq = max_seq;
                    }
                }
                Err(e) => {
                    eprintln!("error fetching: {}", e);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    } else {
        // One-shot mode
        match fetch_messages(client, &state.run_id, since).await {
            Ok((events, _)) => {
                if events.is_empty() {
                    println!("No new messages.");
                } else {
                    for event in events {
                        print_event(&event);
                    }
                }
            }
            Err(e) => {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

async fn fetch_messages(
    client: &HotwiredClient,
    run_id: &str,
    since: Option<i64>,
) -> Result<(Vec<serde_json::Value>, i64), String> {
    match client
        .request(
            "get_conversation_events",
            serde_json::json!({
                "run_id": run_id,
                "since_sequence": since,
                "limit": 20,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();
            let events = data
                .get("events")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let max_seq = events
                .iter()
                .filter_map(|e| e.get("sequence").and_then(|v| v.as_i64()))
                .max()
                .unwrap_or(0);
            Ok((events, max_seq))
        }
        Ok(response) => Err(response.error.unwrap_or_else(|| "unknown error".into())),
        Err(e) => Err(e.to_string()),
    }
}

fn print_event(event: &serde_json::Value) {
    let source = event.get("source").and_then(|v| v.as_str()).unwrap_or("?");
    let event_type = event.get("eventType").and_then(|v| v.as_str()).unwrap_or("message");
    let content = event
        .get("content")
        .and_then(|v| v.as_str())
        .or_else(|| event.get("summary").and_then(|v| v.as_str()))
        .unwrap_or("");
    let timestamp = event.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");

    println!("[{}] {}→{}", format_timestamp(timestamp), source, event_type);
    if !content.is_empty() {
        // Indent content for readability
        for line in content.lines() {
            println!("  {}", line);
        }
    }
    println!();
}
```

### D.5 complete.rs - Mark task complete

```rust
// commands/complete.rs
use crate::ipc::HotwiredClient;
use super::{handle_error, validate};

pub async fn run(client: &HotwiredClient, outcome: Option<String>) {
    // Validate session first
    let state = validate::require_session(client).await;

    match client
        .request(
            "task_complete",
            serde_json::json!({
                "run_id": state.run_id,
                "source": state.role_id,
                "outcome": outcome.unwrap_or_else(|| "Completed".to_string()),
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            println!("Task marked complete.");
            if let Some(data) = response.data {
                if let Some(next) = data.get("next_action").and_then(|v| v.as_str()) {
                    println!("Next: {}", next);
                }
            }
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "failed to complete".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}
```

### D.6 impediment.rs - Report a blocker

```rust
// commands/impediment.rs
use crate::ipc::HotwiredClient;
use super::{handle_error, validate};

pub async fn run(
    client: &HotwiredClient,
    description: &str,
    impediment_type: &str,
    suggestion: Option<String>,
) {
    // Validate session first
    let state = validate::require_session(client).await;

    match client
        .request(
            "report_impediment",
            serde_json::json!({
                "run_id": state.run_id,
                "source": state.role_id,
                "type": impediment_type,
                "description": description,
                "suggestion": suggestion,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            println!("Impediment reported.");
            println!();
            println!("Type: {}", impediment_type);
            println!("Description: {}", description);
            if let Some(ref s) = suggestion {
                println!("Suggestion: {}", s);
            }
            println!();
            println!("The human operator has been notified.");
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "failed to report".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}
```

### D.7 status.rs - Check current run status

```rust
// commands/status.rs
use crate::ipc::HotwiredClient;
use super::{handle_error, validate};

pub async fn run(client: &HotwiredClient) {
    // Validate session first
    let state = validate::require_session(client).await;

    // Get detailed run info
    match client
        .request(
            "get_run_status",
            serde_json::json!({
                "run_id": state.run_id,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();

            let status = data.get("status").and_then(|v| v.as_str()).unwrap_or("-");
            let phase = data.get("phase").and_then(|v| v.as_str()).unwrap_or("-");
            let playbook = data.get("playbook").and_then(|v| v.as_str()).unwrap_or("-");

            println!("Run:      {}", state.run_id);
            println!("Status:   {}", status);
            println!("Phase:    {}", phase);
            println!("Playbook: {}", playbook);
            println!("My Role:  {} ({})", state.role_id, state.run_status);
            println!();

            // Print connected agents
            if let Some(agents) = data.get("agents").and_then(|v| v.as_array()) {
                println!("Connected Agents:");
                for agent in agents {
                    let role = agent.get("role").and_then(|v| v.as_str()).unwrap_or("-");
                    let agent_status = agent.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                    let is_me = role == state.role_id;
                    println!(
                        "  - {} {} - {}",
                        role,
                        if is_me { "(me)" } else { "" },
                        agent_status
                    );
                }
            }
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "failed to get status".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}
```

---

## Appendix E: Updated main.rs with All Commands

```rust
// main.rs - Complete with all new commands

mod commands;
mod ipc;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hotwired")]
#[command(about = "CLI for Hotwired multi-agent workflow orchestration")]
#[command(disable_version_flag = true)]
struct Args {
    #[arg(long, short = 'V')]
    version: bool,

    #[arg(long, short = 's', global = true)]
    socket_path: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    // === Existing management commands ===

    /// Manage workflow runs
    Run {
        #[command(subcommand)]
        action: RunAction,
    },

    /// Manage agent sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Authentication and connection status
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    // === NEW: Workflow commands (top-level, no nesting) ===

    /// Start a new workflow run
    ///
    /// Initializes a new Hotwired workflow. The terminal becomes attached
    /// to the run and receives the protocol instructions.
    ///
    /// Examples:
    ///   hotwired hotwire --intent "Build user authentication"
    ///   hotwired hotwire --playbook architect-team --intent "Implement OAuth"
    Hotwire {
        /// Playbook to use (e.g., plan-build, architect-team)
        #[arg(long)]
        playbook: Option<String>,

        /// What you want to accomplish
        #[arg(long)]
        intent: Option<String>,

        /// Project directory (defaults to current dir)
        #[arg(long)]
        project: Option<PathBuf>,
    },

    /// Join an existing workflow run
    ///
    /// Attaches this terminal to an existing run. You'll receive the
    /// protocol instructions for your assigned role.
    ///
    /// Examples:
    ///   hotwired pair abc123
    ///   hotwired pair abc123 --role worker-1
    Pair {
        /// Run ID to join
        run_id: String,

        /// Role to take (e.g., worker-1, builder)
        #[arg(long)]
        role: Option<String>,
    },

    /// Send a message to another participant
    ///
    /// Sends a handoff or message to another agent or the human operator.
    ///
    /// Examples:
    ///   hotwired send --to orchestrator "Task 1.1 complete"
    ///   hotwired send --to human "Need clarification on auth approach"
    Send {
        /// Recipient: orchestrator, implementer, human, or role ID
        #[arg(long)]
        to: String,

        /// Message content
        #[arg(trailing_var_arg = true)]
        message: Vec<String>,
    },

    /// Check for incoming messages
    ///
    /// Retrieves recent messages from the conversation.
    ///
    /// Examples:
    ///   hotwired inbox
    ///   hotwired inbox --watch
    ///   hotwired inbox --since 42
    Inbox {
        /// Continuously watch for new messages
        #[arg(long)]
        watch: bool,

        /// Only show messages after this sequence number
        #[arg(long)]
        since: Option<i64>,
    },

    /// Mark the current task as complete
    ///
    /// Signals that your assigned work is done.
    ///
    /// Examples:
    ///   hotwired complete
    ///   hotwired complete --outcome "All tests passing"
    Complete {
        /// Description of the outcome
        #[arg(long)]
        outcome: Option<String>,
    },

    /// Report a blocker/impediment
    ///
    /// Signals that you're stuck and need help.
    ///
    /// Examples:
    ///   hotwired impediment "Cannot access database"
    ///   hotwired impediment "Need push access" --type access --suggestion "Grant write perms"
    Impediment {
        /// Description of the blocker
        description: String,

        /// Type: technical, access, clarification, decision
        #[arg(long, default_value = "technical")]
        r#type: String,

        /// Suggested resolution
        #[arg(long)]
        suggestion: Option<String>,
    },

    /// Check current run status
    ///
    /// Shows the status of the attached run and connected agents.
    Status,
}

// ... RunAction, SessionAction, AuthAction enums unchanged ...

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.version {
        commands::print_version(args.socket_path).await;
        return Ok(());
    }

    let client = ipc::HotwiredClient::new(args.socket_path);

    match args.command {
        // Existing commands
        Some(Commands::Run { action }) => match action {
            RunAction::List => commands::run::list(&client).await,
            RunAction::Show { id } => commands::run::show(&client, &id).await,
            RunAction::Remove { id } => commands::run::remove(&client, &id).await,
        },
        Some(Commands::Session { action }) => match action {
            SessionAction::List => commands::session::list(&client).await,
            SessionAction::Show { name } => commands::session::show(&client, &name).await,
            SessionAction::Remove { name } => commands::session::remove(&client, &name).await,
        },
        Some(Commands::Auth { action }) => match action {
            AuthAction::Status => commands::auth::status(&client).await,
        },

        // NEW: Workflow commands
        Some(Commands::Hotwire { playbook, intent, project }) => {
            commands::hotwire::run(&client, playbook, intent, project).await;
        }
        Some(Commands::Pair { run_id, role }) => {
            commands::pair::run(&client, &run_id, role.as_deref()).await;
        }
        Some(Commands::Send { to, message }) => {
            let msg = message.join(" ");
            commands::send::run(&client, &to, &msg).await;
        }
        Some(Commands::Inbox { watch, since }) => {
            commands::inbox::run(&client, watch, since).await;
        }
        Some(Commands::Complete { outcome }) => {
            commands::complete::run(&client, outcome).await;
        }
        Some(Commands::Impediment { description, r#type, suggestion }) => {
            commands::impediment::run(&client, &description, &r#type, suggestion).await;
        }
        Some(Commands::Status) => {
            commands::status::run(&client).await;
        }

        None => {
            use clap::CommandFactory;
            Args::command().print_help()?;
            println!();
        }
    }

    Ok(())
}
```

---

## Appendix F: Artifact Commands (Simplified)

Replaces the 14+ doc-artifact MCP tools with 6 lightweight CLI commands. See Issue #13 and `docs/features/COMMENT_ANCHORING.md` for background.

### F.1 Design Principles

1. **No content in requests/responses** - Agent edits files with normal Write tool
2. **Path-based interface** - Users work with file paths, not opaque IDs
3. **Explicit move** - Moving files without `artifact mv` orphans comments (honest, no magic)
4. **Text-anchored comments** - Comments attach to text content, not line numbers

### F.2 Commands

```bash
hotwired artifact ls                              # List all tracked artifacts (shows missing status)
hotwired artifact sync <path>                     # Register new or update existing (creates version)
hotwired artifact mv <old-path> <new-path>        # Move file AND update refs
hotwired artifact mv <old-path> <new-path> --refs-only  # Only update refs (file already moved)
hotwired artifact comment <path> "<text>" "<msg>" # Add comment anchored to text
hotwired artifact comments <path>                 # List comments on artifact
hotwired artifact resolve <comment-id>            # Resolve a comment
hotwired artifact versions <path>                 # List all versions of artifact
hotwired artifact version <path> <version>        # Show specific version content
```

### F.3 Workflow Example

```bash
# Agent edits a document normally
# (uses Write tool, not special MCP calls)

# Sync to register/update artifact and relocate comment anchors
# This stores a versioned snapshot of the document
hotwired artifact sync docs/PRD.md
# → Artifact registered: docs/PRD.md
# → Title: "Product Requirements Document"
# → Version: 1
# → 3 comments relocated, 0 orphaned

# Add a comment anchored to specific text
hotwired artifact comment docs/PRD.md "authentication flow" "Consider OAuth2 instead of JWT"
# → Comment added: cmt_abc123

# List comments
hotwired artifact comments docs/PRD.md
# → [cmt_abc123] "authentication flow" - "Consider OAuth2 instead of JWT" (open)
# → [cmt_def456] "rate limiting" - "Add to MVP scope?" (open)

# Resolve a comment
hotwired artifact resolve cmt_abc123
# → Comment resolved

# Check version history
hotwired artifact versions docs/PRD.md
# → VERSION  TIMESTAMP            CHANGES
# → 3        2024-01-15 14:30:00  +50 -12 lines
# → 2        2024-01-15 13:15:00  +120 -5 lines
# → 1        2024-01-15 10:00:00  (initial)

# View a specific version
hotwired artifact version docs/PRD.md 2
# → [Full document content at version 2]

# If moving the file - use artifact mv to preserve comments
# This MOVES the file AND updates refs
hotwired artifact mv docs/PRD.md docs/specs/PRD.md
# → File moved: docs/PRD.md → docs/specs/PRD.md
# → Artifact refs updated, 2 comments preserved
```

### F.3.1 Recovery Workflow (File Already Moved)

```bash
# User realizes comments are gone on a file they moved
# They ask Claude to help

# Claude lists artifacts, sees old path with "missing" status
hotwired artifact ls
# → PATH                  COMMENTS  STATUS    TITLE                    VERSIONS
# → docs/PRD.md           3         missing   Product Requirements     5
# → docs/DESIGN.md        1         ok        System Design            2

# Claude figures out where file went, updates refs only
hotwired artifact mv docs/PRD.md docs/specs/PRD.md --refs-only
# → Artifact refs updated: docs/PRD.md → docs/specs/PRD.md
# → 3 comments preserved
# → (file not moved - already at new location)

# Now comments work at new path
hotwired artifact comments docs/specs/PRD.md
# → [cmt_abc123] "authentication flow" - "Consider OAuth2" (open)
```

### F.4 Backend Requirements

New socket handlers needed:

```rust
"artifact_list" => {
    // List all tracked artifacts for the current run
    // Check if file exists at path, set status accordingly
    // Returns: [{ path, artifact_id, comment_count, status: "ok"|"missing", title, version_count, last_synced }]
}

"artifact_sync" => {
    // Register new artifact OR diff existing and relocate comments
    // MUST:
    //   1. Store the path
    //   2. Extract title from first `# header` in document
    //   3. Store ENTIRE document content as new version
    //   4. Diff against previous version, relocate comment anchors
    // Params: { run_id, path }
    // Returns: { artifact_id, status: "registered"|"synced", title, version, comments_relocated, comments_orphaned }
}

"artifact_move" => {
    // Move artifact to new path
    // Params: { run_id, old_path, new_path, refs_only: bool }
    //
    // Validation:
    //   - old_path MUST exist in artifacts table (error if not - sync first!)
    //
    // Behavior:
    //   - If refs_only=false (default):
    //       1. Actually move the file on disk (old_path → new_path)
    //       2. Update artifact path in DB
    //   - If refs_only=true:
    //       1. Verify new_path exists on disk (error if not)
    //       2. Only update artifact path in DB (file already moved)
    //
    // Returns: { artifact_id, comments_preserved, file_moved: bool }
}

"artifact_list_versions" => {
    // List all versions of an artifact
    // Params: { run_id, path }
    // Returns: [{ version, timestamp, lines_added, lines_removed }]
}

"artifact_get_version" => {
    // Get specific version content
    // Params: { run_id, path, version }
    // Returns: { version, timestamp, title, content }
}

"artifact_add_comment" => {
    // Add text-anchored comment
    // Params: { run_id, path, target_text, comment, author }
    // Returns: { comment_id }
}

"artifact_list_comments" => {
    // List comments on artifact
    // Params: { run_id, path, status_filter? }
    // Returns: [{ comment_id, target_text, comment, author, status }]
}

"artifact_resolve_comment" => {
    // Resolve a comment
    // Params: { run_id, comment_id, resolved_by }
    // Returns: { status: "resolved" }
}
```

### F.4.1 Database Schema for Versioning

```sql
-- Artifact versions table
CREATE TABLE artifact_versions (
    id TEXT PRIMARY KEY,
    artifact_id TEXT NOT NULL REFERENCES artifacts(id),
    version INTEGER NOT NULL,
    title TEXT,                    -- Extracted from first # header
    content TEXT NOT NULL,         -- Full document content
    content_hash TEXT NOT NULL,    -- For quick comparison
    lines_added INTEGER,           -- Diff stats vs previous
    lines_removed INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(artifact_id, version)
);

-- Index for quick version lookups
CREATE INDEX idx_artifact_versions_artifact ON artifact_versions(artifact_id, version DESC);
```

### F.4.2 Why Store Full Content?

Storing full document content on each sync enables:
1. **Debugging** - See exactly what agents changed over time
2. **Diff generation** - Compare any two versions
3. **Comment relocation** - Diff old→new to relocate anchors
4. **Rollback potential** - Could restore previous versions if needed
5. **Audit trail** - Complete history for observability

### F.5 Comment Anchoring (Backend Implementation)

See `docs/features/COMMENT_ANCHORING.md` for full design. Key points:

```rust
struct CommentAnchor {
    target_text: String,        // The exact text commented on
    prefix_context: String,     // ~50 chars before (for relocation)
    suffix_context: String,     // ~50 chars after
    hint_line: u32,             // Approximate position (optimization)
}
```

On `artifact_sync`:
1. Diff old snapshot vs current file content
2. For each comment, try to relocate using target_text + context
3. If text gone → mark orphaned → return in response
4. Store new snapshot

### F.6 CLI Implementation

```rust
// commands/artifact.rs

#[derive(Subcommand)]
pub enum ArtifactAction {
    /// List all tracked artifacts in the current run
    #[command(alias = "ls")]
    List,

    /// Sync a file (register new or update existing)
    Sync {
        /// Path to the file
        path: PathBuf,
    },

    /// Move an artifact to a new path (preserves comments)
    ///
    /// By default, this command MOVES the file on disk AND updates the artifact refs.
    /// Use --refs-only if the file was already moved and you just need to update refs.
    ///
    /// NOTE: The artifact must already be synced. If not, run `artifact sync` first.
    ///
    /// Examples:
    ///   hotwired artifact mv docs/old.md docs/new.md           # Move file + update refs
    ///   hotwired artifact mv docs/old.md docs/new.md --refs-only  # Just update refs
    #[command(alias = "mv")]
    Move {
        /// Current path (where artifact is registered)
        old_path: PathBuf,
        /// New path
        new_path: PathBuf,
        /// Only update refs, don't move the file (use when file already moved)
        #[arg(long)]
        refs_only: bool,
    },

    /// List all versions of an artifact
    ///
    /// Shows version history with timestamps and change stats.
    /// Useful for debugging agent changes over time.
    Versions {
        /// Path to the artifact
        path: PathBuf,
    },

    /// Show content of a specific version
    ///
    /// Retrieves the full document content at a specific version.
    Version {
        /// Path to the artifact
        path: PathBuf,
        /// Version number
        version: u32,
    },

    /// Add a comment anchored to specific text
    Comment {
        /// Path to the artifact
        path: PathBuf,
        /// Text to anchor the comment to
        target_text: String,
        /// Comment message
        message: String,
    },

    /// List comments on an artifact
    Comments {
        /// Path to the artifact
        path: PathBuf,
        /// Filter by status: open, resolved, all
        #[arg(long, default_value = "open")]
        status: String,
    },

    /// Resolve a comment
    Resolve {
        /// Comment ID
        comment_id: String,
    },
}

pub async fn list(client: &HotwiredClient) {
    let state = validate::require_session(client).await;

    match client
        .request("artifact_list", json!({ "run_id": state.run_id }))
        .await
    {
        Ok(response) if response.success => {
            let artifacts = response
                .data
                .as_ref()
                .and_then(|d| d.get("artifacts"))
                .and_then(|a| a.as_array())
                .cloned()
                .unwrap_or_default();

            if artifacts.is_empty() {
                println!("No tracked artifacts.");
                return;
            }

            println!("{:<30} {:<8} {:<8} {:<8} {:<20}", "PATH", "STATUS", "COMMENTS", "VERSIONS", "TITLE");
            for a in &artifacts {
                let path = a.get("path").and_then(|v| v.as_str()).unwrap_or("-");
                let status = a.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                let comments = a.get("comment_count").and_then(|v| v.as_i64()).unwrap_or(0);
                let versions = a.get("version_count").and_then(|v| v.as_i64()).unwrap_or(0);
                let title = a.get("title").and_then(|v| v.as_str()).unwrap_or("-");

                // Truncate title if too long
                let title_display = if title.len() > 20 {
                    format!("{}...", &title[..17])
                } else {
                    title.to_string()
                };

                // Color-code status (in actual impl, use terminal colors)
                let status_display = match status {
                    "ok" => "ok",
                    "missing" => "MISSING",  // Would be red in real output
                    _ => status,
                };

                println!("{:<30} {:<8} {:<8} {:<8} {:<20}", path, status_display, comments, versions, title_display);
            }
        }
        Ok(response) => {
            eprintln!("error: {}", response.error.unwrap_or_else(|| "unknown".into()));
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

pub async fn sync(client: &HotwiredClient, path: &Path) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_sync",
            json!({
                "run_id": state.run_id,
                "path": path.to_string_lossy(),
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();
            let status = data.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
            let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled");
            let version = data.get("version").and_then(|v| v.as_i64()).unwrap_or(1);
            let relocated = data.get("comments_relocated").and_then(|v| v.as_i64()).unwrap_or(0);
            let orphaned = data.get("comments_orphaned").and_then(|v| v.as_i64()).unwrap_or(0);

            match status {
                "registered" => {
                    println!("Artifact registered: {}", path.display());
                    println!("  Title: {}", title);
                    println!("  Version: {}", version);
                }
                "synced" => {
                    println!("Artifact synced: {}", path.display());
                    println!("  Title: {}", title);
                    println!("  Version: {}", version);
                    if relocated > 0 || orphaned > 0 {
                        println!("  {} comments relocated, {} orphaned", relocated, orphaned);
                    }
                }
                _ => println!("Status: {}", status),
            }
        }
        Ok(response) => {
            eprintln!("error: {}", response.error.unwrap_or_else(|| "unknown".into()));
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

pub async fn move_artifact(client: &HotwiredClient, old_path: &Path, new_path: &Path, refs_only: bool) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_move",
            json!({
                "run_id": state.run_id,
                "old_path": old_path.to_string_lossy(),
                "new_path": new_path.to_string_lossy(),
                "refs_only": refs_only,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();
            let preserved = data.get("comments_preserved").and_then(|v| v.as_i64()).unwrap_or(0);
            let file_moved = data.get("file_moved").and_then(|v| v.as_bool()).unwrap_or(false);

            if file_moved {
                println!("File moved: {} → {}", old_path.display(), new_path.display());
            }
            println!("Artifact refs updated: {} → {}", old_path.display(), new_path.display());
            println!("  {} comments preserved", preserved);
        }
        Ok(response) => {
            // Backend should return specific errors:
            // - "artifact_not_found" if old_path not in artifacts table
            // - "file_not_found" if refs_only but new_path doesn't exist
            eprintln!("error: {}", response.error.unwrap_or_else(|| "unknown".into()));
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

pub async fn list_versions(client: &HotwiredClient, path: &Path) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_list_versions",
            json!({
                "run_id": state.run_id,
                "path": path.to_string_lossy(),
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let versions = response
                .data
                .as_ref()
                .and_then(|d| d.get("versions"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if versions.is_empty() {
                println!("No versions found. Run `artifact sync` first.");
                return;
            }

            println!("{:<8} {:<20} {}", "VERSION", "TIMESTAMP", "CHANGES");
            for v in &versions {
                let version = v.get("version").and_then(|x| x.as_i64()).unwrap_or(0);
                let timestamp = v.get("timestamp").and_then(|x| x.as_str()).unwrap_or("-");
                let added = v.get("lines_added").and_then(|x| x.as_i64()).unwrap_or(0);
                let removed = v.get("lines_removed").and_then(|x| x.as_i64()).unwrap_or(0);

                let changes = if version == 1 {
                    "(initial)".to_string()
                } else {
                    format!("+{} -{} lines", added, removed)
                };

                println!("{:<8} {:<20} {}", version, format_timestamp(timestamp), changes);
            }
        }
        Ok(response) => {
            eprintln!("error: {}", response.error.unwrap_or_else(|| "unknown".into()));
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

pub async fn get_version(client: &HotwiredClient, path: &Path, version: u32) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_get_version",
            json!({
                "run_id": state.run_id,
                "path": path.to_string_lossy(),
                "version": version,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();
            let title = data.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled");
            let timestamp = data.get("timestamp").and_then(|v| v.as_str()).unwrap_or("-");
            let content = data.get("content").and_then(|v| v.as_str()).unwrap_or("");

            println!("# {} (version {})", title, version);
            println!("# Synced: {}", format_timestamp(timestamp));
            println!("# {}", "-".repeat(60));
            println!();
            println!("{}", content);
        }
        Ok(response) => {
            eprintln!("error: {}", response.error.unwrap_or_else(|| "unknown".into()));
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

pub async fn add_comment(client: &HotwiredClient, path: &Path, target_text: &str, message: &str) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_add_comment",
            json!({
                "run_id": state.run_id,
                "path": path.to_string_lossy(),
                "target_text": target_text,
                "comment": message,
                "author": state.role_id,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();
            let comment_id = data.get("comment_id").and_then(|v| v.as_str()).unwrap_or("?");
            println!("Comment added: {}", comment_id);
        }
        Ok(response) => {
            eprintln!("error: {}", response.error.unwrap_or_else(|| "unknown".into()));
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

pub async fn list_comments(client: &HotwiredClient, path: &Path, status_filter: &str) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_list_comments",
            json!({
                "run_id": state.run_id,
                "path": path.to_string_lossy(),
                "status_filter": status_filter,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let comments = response
                .data
                .as_ref()
                .and_then(|d| d.get("comments"))
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();

            if comments.is_empty() {
                println!("No comments.");
                return;
            }

            for c in &comments {
                let id = c.get("comment_id").and_then(|v| v.as_str()).unwrap_or("?");
                let target = c.get("target_text").and_then(|v| v.as_str()).unwrap_or("");
                let msg = c.get("comment").and_then(|v| v.as_str()).unwrap_or("");
                let status = c.get("status").and_then(|v| v.as_str()).unwrap_or("?");

                let target_preview = if target.len() > 30 {
                    format!("{}...", &target[..30])
                } else {
                    target.to_string()
                };

                println!("[{}] \"{}\" - {} ({})", id, target_preview, msg, status);
            }
        }
        Ok(response) => {
            eprintln!("error: {}", response.error.unwrap_or_else(|| "unknown".into()));
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

pub async fn resolve(client: &HotwiredClient, comment_id: &str) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_resolve_comment",
            json!({
                "run_id": state.run_id,
                "comment_id": comment_id,
                "resolved_by": state.role_id,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            println!("Comment resolved: {}", comment_id);
        }
        Ok(response) => {
            eprintln!("error: {}", response.error.unwrap_or_else(|| "unknown".into()));
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}
```

### F.7 File Changes

```
src/commands/
├── artifact.rs     # NEW - artifact subcommands (ls, sync, mv, comment, comments, resolve, versions, version)
└── mod.rs          # Add: pub mod artifact;

src/main.rs         # Add Artifact { action: ArtifactAction } subcommand
```

**main.rs routing:**
```rust
Some(Commands::Artifact { action }) => match action {
    ArtifactAction::List => commands::artifact::list(&client).await,
    ArtifactAction::Sync { path } => commands::artifact::sync(&client, &path).await,
    ArtifactAction::Move { old_path, new_path, refs_only } => {
        commands::artifact::move_artifact(&client, &old_path, &new_path, refs_only).await
    }
    ArtifactAction::Comment { path, target_text, message } => {
        commands::artifact::add_comment(&client, &path, &target_text, &message).await
    }
    ArtifactAction::Comments { path, status } => {
        commands::artifact::list_comments(&client, &path, &status).await
    }
    ArtifactAction::Resolve { comment_id } => {
        commands::artifact::resolve(&client, &comment_id).await
    }
    ArtifactAction::Versions { path } => {
        commands::artifact::list_versions(&client, &path).await
    }
    ArtifactAction::Version { path, version } => {
        commands::artifact::get_version(&client, &path, version).await
    }
}
```

### F.8 Replaces These MCP Tools

All 14+ doc-artifact MCP tools are replaced:

| Old MCP Tool | Replaced By |
|--------------|-------------|
| doc_artifact_list | `hotwired artifact ls` |
| doc_artifact_create | `hotwired artifact sync` (auto-registers) |
| doc_artifact_read | Normal `Read` tool (no special call needed) |
| doc_artifact_edit | Normal `Write` tool + `artifact sync` |
| doc_artifact_search | Normal `Grep` tool |
| doc_artifact_add_comment | `hotwired artifact comment` |
| doc_artifact_list_comments | `hotwired artifact comments` |
| doc_artifact_resolve_comment | `hotwired artifact resolve` |
| doc_artifact_suggest_edit | REMOVED (just edit the file) |
| doc_artifact_accept_suggestion | REMOVED |
| doc_artifact_reject_suggestion | REMOVED |
| doc_artifact_list_suggestions | REMOVED |

The "suggestion" workflow is eliminated. Agents just edit files directly.
