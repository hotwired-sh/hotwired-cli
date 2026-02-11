pub mod auth;
pub mod run;
pub mod session;

use crate::ipc::{HotwiredClient, IpcError};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn print_version(socket_path: Option<String>) {
    let client = HotwiredClient::new(socket_path);
    let core_version = match client.health_check().await {
        Ok(response) if response.success => response
            .data
            .as_ref()
            .and_then(|d| d.get("version"))
            .and_then(|v| v.as_str())
            .map(String::from),
        _ => None,
    };

    match core_version {
        Some(v) => println!("hotwired-cli {} (core {})", VERSION, v),
        None => println!(
            "hotwired-cli {} (not connected - is Hotwired.sh desktop app running?)",
            VERSION
        ),
    }
}

pub fn format_timestamp(ts: &str) -> String {
    ts.replace('T', " ").trim_end_matches('Z').to_string()
}

pub fn handle_error(e: IpcError) -> ! {
    match e {
        IpcError::NotConnected(_) => {
            eprintln!("error: not connected - is Hotwired.sh desktop app running?");
        }
        _ => {
            eprintln!("error: {}", e);
        }
    }
    std::process::exit(1);
}
