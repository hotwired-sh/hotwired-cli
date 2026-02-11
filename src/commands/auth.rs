use crate::ipc::{HotwiredClient, IpcError};

pub async fn status(client: &HotwiredClient) {
    let socket_display = client.socket_path().to_string();

    match client.health_check().await {
        Ok(response) if response.success => {
            let version = response
                .data
                .as_ref()
                .and_then(|d| d.get("version"))
                .and_then(|v| v.as_str());

            match version {
                Some(v) => println!("Backend:    running (v{})", v),
                None => println!("Backend:    running"),
            }
        }
        Ok(_) => {
            println!("Backend:    not responding");
        }
        Err(IpcError::NotConnected(_)) => {
            println!("Backend:    not running");
        }
        Err(_) => {
            println!("Backend:    connection failed");
        }
    }

    if std::path::Path::new(&socket_display).exists() {
        println!("Socket:     {}", socket_display);
    } else {
        println!("Socket:     {} (not found)", socket_display);
    }

    let token_path = dirs::home_dir()
        .map(|h| h.join(".hotwired").join("auth_token"))
        .unwrap_or_default();

    if token_path.exists() {
        match std::fs::read_to_string(&token_path) {
            Ok(content) if !content.trim().is_empty() => {
                println!("Auth token: configured");
            }
            _ => {
                println!("Auth token: empty");
            }
        }
    } else {
        println!("Auth token: not configured");
    }
}
