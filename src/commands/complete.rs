//! Mark task as complete
//!
//! The `complete` command signals that the assigned work is done.

use super::{handle_error, validate};
use crate::ipc::HotwiredClient;

pub async fn run(client: &HotwiredClient, outcome: Option<String>) {
    // Validate session first
    let state = validate::require_session(client).await;

    match client
        .request(
            "task_complete",
            serde_json::json!({
                "runId": state.run_id,
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
                response
                    .error
                    .unwrap_or_else(|| "failed to complete".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would require a mock server
}
