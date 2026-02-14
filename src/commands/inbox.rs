//! Check for incoming messages
//!
//! The `inbox` command retrieves recent messages from the conversation.
//! Supports one-shot and continuous watch modes.

use super::{format_timestamp, validate};
use crate::ipc::HotwiredClient;

pub async fn run(client: &HotwiredClient, watch: bool, since: Option<i64>) {
    // Validate session first
    let state = validate::require_session(client).await;

    if watch {
        // Continuous polling mode
        let mut last_seq = since.unwrap_or(0);
        println!("Watching for messages... (Ctrl+C to stop)");
        println!();

        loop {
            match fetch_messages(client, &state.run_id, Some(last_seq)).await {
                Ok((events, max_seq)) => {
                    for event in events {
                        print_event(&event);
                    }
                    if max_seq > last_seq {
                        last_seq = max_seq;
                    }
                }
                Err(e) => {
                    eprintln!("error fetching: {}", e);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    } else {
        // One-shot mode
        match fetch_messages(client, &state.run_id, since).await {
            Ok((events, _)) => {
                if events.is_empty() {
                    println!("No new messages.");
                } else {
                    for event in events {
                        print_event(&event);
                    }
                }
            }
            Err(e) => {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

async fn fetch_messages(
    client: &HotwiredClient,
    run_id: &str,
    since: Option<i64>,
) -> Result<(Vec<serde_json::Value>, i64), String> {
    match client
        .request(
            "get_conversation_events",
            serde_json::json!({
                "run_id": run_id,
                "since_sequence": since,
                "limit": 20,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();
            let events = data
                .get("events")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let max_seq = events
                .iter()
                .filter_map(|e| e.get("sequence").and_then(|v| v.as_i64()))
                .max()
                .unwrap_or(0);
            Ok((events, max_seq))
        }
        Ok(response) => Err(response.error.unwrap_or_else(|| "unknown error".into())),
        Err(e) => Err(e.to_string()),
    }
}

fn print_event(event: &serde_json::Value) {
    let source = event.get("source").and_then(|v| v.as_str()).unwrap_or("?");
    let event_type = event
        .get("eventType")
        .and_then(|v| v.as_str())
        .unwrap_or("message");
    let content = event
        .get("content")
        .and_then(|v| v.as_str())
        .or_else(|| event.get("summary").and_then(|v| v.as_str()))
        .unwrap_or("");
    let timestamp = event
        .get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    println!(
        "[{}] {}→{}",
        format_timestamp(timestamp),
        source,
        event_type
    );
    if !content.is_empty() {
        // Indent content for readability
        for line in content.lines() {
            println!("  {}", line);
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_event_format() {
        let event = serde_json::json!({
            "source": "strategist",
            "eventType": "handoff",
            "content": "Task complete",
            "timestamp": "2024-01-15T10:30:00Z"
        });
        // This would print to stdout - in real tests we'd capture output
        // For now just verify it doesn't panic
        print_event(&event);
    }
}
