use super::{format_timestamp, handle_error};
use crate::ipc::HotwiredClient;

async fn resolve_id(client: &HotwiredClient, short_id: &str) -> String {
    // Full UUIDs (with or without dashes) pass through directly
    if short_id.len() >= 32 {
        return short_id.to_string();
    }

    match client.request("list_runs", serde_json::json!({})).await {
        Ok(response) if response.success => {
            let runs = response
                .data
                .as_ref()
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();

            let matches: Vec<String> = runs
                .iter()
                .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(String::from))
                .filter(|id| id.starts_with(short_id))
                .collect();

            match matches.len() {
                0 => {
                    eprintln!("error: no run matching '{}'", short_id);
                    std::process::exit(1);
                }
                1 => matches.into_iter().next().unwrap(),
                _ => {
                    eprintln!("error: ambiguous run id '{}', be more specific", short_id);
                    std::process::exit(1);
                }
            }
        }
        Ok(_) => {
            eprintln!("error: failed to resolve run id");
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

fn short_id(id: &str) -> &str {
    &id[..id.len().min(8)]
}

pub async fn list(client: &HotwiredClient) {
    match client.request("list_runs", serde_json::json!({})).await {
        Ok(response) if response.success => {
            let runs = response
                .data
                .as_ref()
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();

            if runs.is_empty() {
                println!("No runs.");
                return;
            }

            println!(
                "{:<10} {:<12} {:<14} {:<24} {}",
                "ID", "STATUS", "PHASE", "PLAYBOOK", "CREATED"
            );

            for run in &runs {
                let id = run.get("id").and_then(|v| v.as_str()).unwrap_or("-");
                let status = run.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                let phase = run.get("phase").and_then(|v| v.as_str()).unwrap_or("-");
                let playbook = run
                    .get("templateName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let created =
                    format_timestamp(run.get("createdAt").and_then(|v| v.as_str()).unwrap_or("-"));

                println!(
                    "{:<10} {:<12} {:<14} {:<24} {}",
                    short_id(id),
                    status,
                    phase,
                    playbook,
                    created
                );
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

pub async fn show(client: &HotwiredClient, id: &str) {
    let full_id = resolve_id(client, id).await;

    match client
        .request("get_run_status", serde_json::json!({"runId": full_id}))
        .await
    {
        Ok(response) if response.success => {
            if let Some(data) = &response.data {
                let run_id = data.get("runId").and_then(|v| v.as_str()).unwrap_or("-");
                let status = data.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                let phase = data.get("phase").and_then(|v| v.as_str()).unwrap_or("-");
                let playbook = data
                    .get("templateName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let has_protocol = data
                    .get("hasProtocol")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                println!("Run:        {}", run_id);
                println!("Status:     {}", status);
                println!("Phase:      {}", phase);
                println!("Playbook:   {}", playbook);
                println!("Protocol:   {}", if has_protocol { "yes" } else { "no" });

                if let Some(agents) = data.get("connectedAgents").and_then(|v| v.as_array()) {
                    if !agents.is_empty() {
                        println!();
                        println!("Agents:");
                        for agent in agents {
                            let role = agent.get("roleId").and_then(|v| v.as_str()).unwrap_or("-");
                            let session = agent
                                .get("sessionName")
                                .and_then(|v| v.as_str())
                                .unwrap_or("-");
                            let agent_type = agent
                                .get("agentType")
                                .and_then(|v| v.as_str())
                                .unwrap_or("-");
                            println!("  {:<16} {:<28} ({})", role, session, agent_type);
                        }
                    }
                }
            }
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "run not found".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

pub async fn remove(client: &HotwiredClient, id: &str) {
    let full_id = resolve_id(client, id).await;

    match client
        .request("delete_run", serde_json::json!({"runId": full_id}))
        .await
    {
        Ok(response) if response.success => {
            println!("Removed run {}", short_id(&full_id));
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
