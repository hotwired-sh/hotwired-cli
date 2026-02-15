//! Send a message to another participant
//!
//! The `send` command sends a handoff or message to another agent or the human operator.
//! Requires an active session attached to a run.

use super::{handle_error, truncate, validate};
use crate::ipc::HotwiredClient;

pub async fn run(client: &HotwiredClient, to: &str, message: &str) {
    // Validate session first
    let state = validate::require_session(client).await;

    match client
        .request(
            "handoff",
            serde_json::json!({
                "runId": state.run_id,
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

#[cfg(test)]
mod tests {
    use super::super::truncate;

    #[test]
    fn test_message_truncation_for_summary() {
        let long_message =
            "This is a very long message that should be truncated for the summary field";
        let summary = truncate(long_message, 50);
        assert!(summary.len() <= 50);
        assert!(summary.ends_with("..."));
    }
}
