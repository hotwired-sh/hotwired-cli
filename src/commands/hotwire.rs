//! Start a new workflow run
//!
//! The `hotwire` command initializes a new Hotwired workflow. This is one of the
//! few commands that does NOT require an existing session - it creates one.

use super::handle_error;
use crate::ipc::HotwiredClient;
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
                "zellijSession": zellij_session,
                "projectPath": project_path,
                "suggestedPlaybook": playbook,
                "intent": intent,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.as_ref().unwrap();
            let status = data
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

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
                    let pending_id = data
                        .get("pendingRunId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-");
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

#[cfg(test)]
mod tests {
    // Integration tests would require a mock server
    // Unit tests for parsing logic can go here
}
