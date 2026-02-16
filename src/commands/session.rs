use super::handle_error;
use crate::ipc::HotwiredClient;

/// Format session status for display - make it human-readable
fn format_status(status: &str) -> &str {
    match status {
        "connected" => "connected",
        "agent_not_running" => "no agent",
        "detached" => "detached",
        "zombie" => "zombie",
        "session_gone" => "gone",
        other => other,
    }
}

pub async fn list(client: &HotwiredClient) {
    match client
        .request("list_active_sessions", serde_json::json!({}))
        .await
    {
        Ok(response) if response.success => {
            let sessions = response
                .data
                .as_ref()
                .and_then(|d| d.get("sessions"))
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();

            if sessions.is_empty() {
                println!("No active sessions.");
                return;
            }

            println!(
                "{:<28} {:<12} {:<12} {:<14} PROJECT",
                "SESSION", "STATUS", "ROLE", "RUN"
            );

            for s in &sessions {
                let name = s.get("sessionName").and_then(|v| v.as_str()).unwrap_or("-");
                let project = s.get("projectDir").and_then(|v| v.as_str()).unwrap_or("-");
                let status = s
                    .get("sessionStatus")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let run_id = s.get("attachedRunId").and_then(|v| v.as_str());
                // Only show role if attached to a run (role is meaningless without one)
                let role = if run_id.is_some() {
                    s.get("roleId").and_then(|v| v.as_str()).unwrap_or("-")
                } else {
                    "-"
                };

                // Truncate run ID for display (first 12 chars)
                let run_display = match run_id {
                    Some(id) if id.len() > 12 => &id[..12],
                    Some(id) => id,
                    None => "-",
                };

                println!(
                    "{:<28} {:<12} {:<12} {:<14} {}",
                    name,
                    format_status(status),
                    role,
                    run_display,
                    project
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

pub async fn show(client: &HotwiredClient, name: &str) {
    match client
        .request("list_active_sessions", serde_json::json!({}))
        .await
    {
        Ok(response) if response.success => {
            let sessions = response
                .data
                .as_ref()
                .and_then(|d| d.get("sessions"))
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();

            let session = sessions
                .iter()
                .find(|s| s.get("sessionName").and_then(|v| v.as_str()) == Some(name));

            match session {
                Some(s) => {
                    let session_name = s.get("sessionName").and_then(|v| v.as_str()).unwrap_or("-");
                    let project = s.get("projectDir").and_then(|v| v.as_str()).unwrap_or("-");
                    let worktree = s
                        .get("isWorktree")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let git_dir = s.get("gitCommonDir").and_then(|v| v.as_str());
                    let status = s
                        .get("sessionStatus")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let run_id = s.get("attachedRunId").and_then(|v| v.as_str());
                    let role = s.get("roleId").and_then(|v| v.as_str());

                    println!("Session:  {}", session_name);
                    println!("Status:   {}", format_status(status));
                    println!("Project:  {}", project);
                    println!("Worktree: {}", if worktree { "yes" } else { "no" });
                    if let Some(dir) = git_dir {
                        println!("Git dir:  {}", dir);
                    }
                    if let Some(rid) = run_id {
                        println!("Run:      {}", rid);
                    }
                    if let Some(r) = role {
                        println!("Role:     {}", r);
                    }
                }
                None => {
                    eprintln!("error: no session '{}'", name);
                    std::process::exit(1);
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

pub async fn remove(client: &HotwiredClient, name: &str) {
    match client
        .request(
            "deregister_session",
            serde_json::json!({"sessionName": name}),
        )
        .await
    {
        Ok(response) if response.success => {
            println!("Removed session {}", name);
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

pub async fn register(client: &HotwiredClient, session: &str, project: &str) {
    match client
        .request(
            "register_session",
            serde_json::json!({
                "sessionName": session,
                "projectDir": project
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            println!("Registered session {}", session);
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

pub async fn deregister(client: &HotwiredClient, session: &str) {
    match client
        .request(
            "deregister_session",
            serde_json::json!({"sessionName": session}),
        )
        .await
    {
        Ok(response) if response.success => {
            println!("Deregistered session {}", session);
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
