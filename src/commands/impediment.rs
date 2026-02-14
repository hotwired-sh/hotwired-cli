//! Report a blocker/impediment
//!
//! The `impediment` command signals that you're stuck and need help.

use super::{handle_error, validate};
use crate::ipc::HotwiredClient;

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

#[cfg(test)]
mod tests {
    #[test]
    fn test_impediment_types() {
        // Valid impediment types
        let valid_types = ["technical", "access", "clarification", "decision"];
        for t in valid_types {
            assert!(!t.is_empty());
        }
    }
}
