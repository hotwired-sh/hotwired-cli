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

            println!("Run:      {}", state.run_id);
            println!("Status:   {}", status);
            println!("Phase:    {}", phase);
            println!("Playbook: {}", playbook);
            println!("My Role:  {} ({})", state.role_id, state.run_status);
            println!();

            // Print connected agents
            if let Some(agents) = data.get("connectedAgents").and_then(|v| v.as_array()) {
                println!("Connected Agents:");
                for agent in agents {
                    let role = agent.get("roleId").and_then(|v| v.as_str()).unwrap_or("-");
                    let session = agent
                        .get("sessionName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-");
                    let is_me = role == state.role_id;
                    println!(
                        "  - {}{} ({})",
                        role,
                        if is_me { " (me)" } else { "" },
                        session
                    );
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
