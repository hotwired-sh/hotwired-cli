//! Session validation for workflow commands
//!
//! Every workflow command (except `hotwire` and `pair`) MUST validate session state first.
//! This module provides the validation logic and user-friendly error messages.

use crate::ipc::{HotwiredClient, IpcError};

/// Current session state from hotwired-core
#[derive(Debug, Clone)]
#[allow(dead_code)] // zellij_session kept for debugging/future use
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
    RunNotActive(String),
    /// IPC error (backend not running, etc.)
    IpcError(IpcError),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::NoZellijSession => write!(f, "Not running in a Zellij session"),
            ValidationError::SessionNotRegistered => write!(f, "Session not registered with Hotwired"),
            ValidationError::NotAttachedToRun => write!(f, "Not attached to any run"),
            ValidationError::RunNotActive(status) => write!(f, "Run is not active (status: {})", status),
            ValidationError::IpcError(e) => write!(f, "IPC error: {}", e),
        }
    }
}

impl std::error::Error for ValidationError {}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_display() {
        assert_eq!(
            ValidationError::NoZellijSession.to_string(),
            "Not running in a Zellij session"
        );
        assert_eq!(
            ValidationError::RunNotActive("completed".to_string()).to_string(),
            "Run is not active (status: completed)"
        );
    }

    #[test]
    fn test_session_state_clone() {
        let state = SessionState {
            zellij_session: "test-session".to_string(),
            run_id: "run-123".to_string(),
            role_id: "strategist".to_string(),
            run_status: "active".to_string(),
        };
        let cloned = state.clone();
        assert_eq!(cloned.run_id, "run-123");
    }
}
