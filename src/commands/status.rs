//! Check current run status
//!
//! The `status` command shows the status of the attached run and connected agents.

use super::{handle_error, validate};
use crate::ipc::HotwiredClient;

pub async fn run(client: &HotwiredClient) {
    // Validate session first
    let state = validate::require_session(client).await;

    // Get detailed run info
    match client
        .request(
            "get_run_status",
            serde_json::json!({
                "runId": state.run_id,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();

            let status = data.get("status").and_then(|v| v.as_str()).unwrap_or("-");
            let phase = data.get("phase").and_then(|v| v.as_str()).unwrap_or("-");
            let playbook = data
                .get("templateName")
                .and_then(|v| v.as_str())
                .unwrap_or("-");

            // Identity block — make it unambiguous who the calling agent is
            println!("YOU ARE:  {}", state.role_id);
            println!(
                "Session:  {} (auto-detected from Zellij)",
                state.zellij_session
            );
            println!();
            println!("Run:      {}", state.run_id);
            println!("Status:   {}", status);
            println!("Phase:    {}", phase);
            println!("Playbook: {}", playbook);
            println!();

            // Print connected agents with clear "you" marker
            if let Some(agents) = data.get("connectedAgents").and_then(|v| v.as_array()) {
                println!("Connected Agents:");
                for agent in agents {
                    let role = agent.get("roleId").and_then(|v| v.as_str()).unwrap_or("-");
                    let is_me = role == state.role_id;
                    if is_me {
                        println!("  > {} (you)", role);
                    } else {
                        println!("  - {}", role);
                    }
                }
            }

            // Show impediments when run is blocked
            if let Some(impediments) = data.get("impediments").and_then(|v| v.as_array()) {
                if !impediments.is_empty() {
                    println!();
                    println!("BLOCKED BY:");
                    for imp in impediments {
                        let source = imp.get("source").and_then(|v| v.as_str()).unwrap_or("-");
                        let desc = imp
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("-");
                        println!("  - [{}]: {}", source, desc);
                    }
                    println!();
                    println!("To resolve: hotwired resolve \"<reason>\"");
                }
            }
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response
                    .error
                    .unwrap_or_else(|| "failed to get status".into())
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
