//! Join an existing workflow run
//!
//! The `pair` command attaches this terminal to an existing run. This is one of the
//! few commands that does NOT require an existing session - it creates the attachment.

use super::handle_error;
use crate::ipc::HotwiredClient;

pub async fn run(client: &HotwiredClient, run_id: &str, role: Option<&str>) {
    // pair does NOT require existing session - it creates the attachment
    let zellij_session = std::env::var("ZELLIJ_SESSION_NAME").ok();

    if zellij_session.is_none() {
        eprintln!("ERROR: Not running in a Zellij session.");
        eprintln!("The hotwired CLI must be run from within a Hotwired-managed terminal.");
        std::process::exit(1);
    }

    match client
        .request(
            "pair",
            serde_json::json!({
                "zellij_session": zellij_session,
                "run_id": run_id,
                "role_id": role,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.as_ref().unwrap();
            let role = data.get("role").and_then(|v| v.as_str()).unwrap_or("-");
            let protocol = data.get("protocol").and_then(|v| v.as_str()).unwrap_or("");

            println!("Joined run: {}", run_id);
            println!("Your role: {}", role);
            println!();
            println!("{}", protocol);
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
}
