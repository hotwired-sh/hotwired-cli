use crate::ipc::HotwiredClient;
use super::handle_error;

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

            println!("{:<28} {:<44} {}", "SESSION", "PROJECT", "WORKTREE");

            for s in &sessions {
                let name = s.get("sessionName").and_then(|v| v.as_str()).unwrap_or("-");
                let project = s
                    .get("projectDir")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let worktree = s
                    .get("isWorktree")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                println!(
                    "{:<28} {:<44} {}",
                    name,
                    project,
                    if worktree { "yes" } else { "no" }
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
                    let session_name =
                        s.get("sessionName").and_then(|v| v.as_str()).unwrap_or("-");
                    let project = s
                        .get("projectDir")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-");
                    let worktree = s
                        .get("isWorktree")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let git_dir = s.get("gitCommonDir").and_then(|v| v.as_str());

                    println!("Session:    {}", session_name);
                    println!("Project:    {}", project);
                    println!("Worktree:   {}", if worktree { "yes" } else { "no" });
                    if let Some(dir) = git_dir {
                        println!("Git dir:    {}", dir);
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
