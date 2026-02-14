pub mod auth;
pub mod run;
pub mod session;
pub mod validate;

// Workflow commands
pub mod hotwire;
pub mod pair;
pub mod send;
pub mod inbox;
pub mod complete;
pub mod impediment;
pub mod status;

// Artifact commands
pub mod artifact;

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

/// Truncate string with ellipsis
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp("2024-01-15T10:30:00Z"), "2024-01-15 10:30:00");
    }
}
