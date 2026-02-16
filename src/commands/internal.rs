//! Internal commands for Claude Code hook integration
//!
//! These commands are called by Claude Code lifecycle hooks (via hooks.json).
//! They are hidden from `--help` and designed to be fire-and-forget:
//! all IPC errors are silently ignored to avoid blocking Claude.

use crate::ipc::HotwiredClient;
use tokio::io::AsyncReadExt;

/// Read stdin with a timeout, returning empty string on failure or timeout.
async fn read_stdin() -> String {
    let result = tokio::time::timeout(std::time::Duration::from_millis(500), async {
        let mut buf = String::new();
        let mut stdin = tokio::io::stdin();
        stdin.read_to_string(&mut buf).await.ok();
        buf
    })
    .await;

    result.unwrap_or_default()
}

/// Parse stdin as JSON, falling back to empty object.
async fn read_stdin_json() -> serde_json::Value {
    let raw = read_stdin().await;
    if raw.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&raw).unwrap_or(serde_json::json!({}))
    }
}

/// Handle a generic hook event (Stop, PreCompact, Notification, SubagentStart, etc.)
pub async fn hook_event(client: &HotwiredClient, event_name: &str) {
    let payload = read_stdin_json().await;
    let zellij_session = std::env::var("ZELLIJ_SESSION_NAME").ok();
    let project_dir = std::env::var("CLAUDE_PROJECT_DIR").ok();

    let _ = client
        .request(
            "hook_event",
            serde_json::json!({
                "eventName": event_name,
                "zellijSession": zellij_session,
                "projectDir": project_dir,
                "payload": payload,
            }),
        )
        .await;
}

/// Handle session-start: register session + fire hook event for logging.
///
/// This intentionally fires two IPC calls:
/// 1. `register_session` - updates session state in DB, broadcasts `session:registered`
/// 2. `hook_event` - broadcasts `hook:session_start` for telemetry logging
pub async fn session_start(client: &HotwiredClient) {
    let zellij_session = std::env::var("ZELLIJ_SESSION_NAME").unwrap_or_default();
    let project_dir = std::env::var("CLAUDE_PROJECT_DIR")
        .or_else(|_| std::env::current_dir().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_default();

    // Skip if not in a Zellij session
    if zellij_session.is_empty() {
        return;
    }

    // Register session (existing IPC method)
    let _ = client
        .request(
            "register_session",
            serde_json::json!({
                "sessionName": zellij_session,
                "projectDir": project_dir,
            }),
        )
        .await;

    // Also fire hook event for telemetry logging
    let _ = client
        .request(
            "hook_event",
            serde_json::json!({
                "eventName": "session_start",
                "zellijSession": zellij_session,
                "projectDir": project_dir,
                "payload": {},
            }),
        )
        .await;
}

/// Handle session-end: deregister session + fire hook event for logging.
///
/// This intentionally fires two IPC calls:
/// 1. `deregister_session` - removes session from DB, broadcasts `session:deregistered`
/// 2. `hook_event` - broadcasts `hook:session_end` for telemetry logging
pub async fn session_end(client: &HotwiredClient) {
    let zellij_session = std::env::var("ZELLIJ_SESSION_NAME").unwrap_or_default();

    // Skip if not in a Zellij session
    if zellij_session.is_empty() {
        return;
    }

    // Deregister session (existing IPC method)
    let _ = client
        .request(
            "deregister_session",
            serde_json::json!({
                "sessionName": zellij_session,
            }),
        )
        .await;

    // Also fire hook event for telemetry logging
    let _ = client
        .request(
            "hook_event",
            serde_json::json!({
                "eventName": "session_end",
                "zellijSession": zellij_session,
                "payload": {},
            }),
        )
        .await;
}
