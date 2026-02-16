//! Fetch protocol instructions for the current run/role
//!
//! The `protocol` command re-fetches the protocol instructions from hotwired-core.
//! This is useful when an agent's context has been compacted or it needs to
//! re-read its instructions.

use super::{handle_error, validate};
use crate::ipc::HotwiredClient;

pub async fn run(client: &HotwiredClient) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "get_protocol",
            serde_json::json!({
                "runId": state.run_id,
                "role": state.role_id,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();

            let run_id = data.get("runId").and_then(|v| v.as_str()).unwrap_or("-");
            let template = data
                .get("templateName")
                .and_then(|v| v.as_str())
                .unwrap_or("-");
            let playbook_protocol = data
                .get("playbookProtocol")
                .and_then(|v| v.as_str())
                .unwrap_or("(No protocol instructions available)");
            let role_protocol = data.get("roleProtocol").and_then(|v| v.as_str());
            let init_condition = data.get("initializationCondition").and_then(|v| v.as_str());

            // Format capabilities
            let capabilities_section = data
                .get("capabilities")
                .and_then(|v| v.as_object())
                .and_then(|caps| {
                    let can_resolve = caps
                        .get("canResolveImpediments")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if can_resolve {
                        Some("- Can resolve impediments: You can use `hotwired impediment` to resolve blockers raised by other agents")
                    } else {
                        None
                    }
                });

            // Print formatted protocol (matches hotwired-mcp format_protocol_response)
            println!("# Hotwired Workflow Protocol");
            println!();
            println!("**Run ID:** {}", run_id);
            println!("**Playbook:** {}", template);
            println!();
            println!("## Protocol Instructions");
            println!();
            println!("{}", playbook_protocol);

            if let Some(rp) = role_protocol {
                if !rp.is_empty() {
                    println!();
                    println!("## Your Role Instructions");
                    println!();
                    println!("{}", rp);
                }
            }

            if let Some(cap) = capabilities_section {
                println!();
                println!("## Your Capabilities");
                println!();
                println!("{}", cap);
            }

            if let Some(ic) = init_condition {
                println!();
                println!("## Initialization Condition");
                println!();
                println!("{}", ic);
            }
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response
                    .error
                    .unwrap_or_else(|| "failed to get protocol".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}
