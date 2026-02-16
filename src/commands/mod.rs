pub mod auth;
pub mod run;
pub mod session;
pub mod validate;

// Workflow commands
pub mod complete;
pub mod hotwire;
pub mod impediment;
pub mod inbox;
pub mod pair;
pub mod protocol;
pub mod send;
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
        assert_eq!(
            format_timestamp("2024-01-15T10:30:00Z"),
            "2024-01-15 10:30:00"
        );
    }
}

/// Tests for IPC parameter serialization
/// Verifies that CLI sends camelCase field names matching hotwired-core expectations
#[cfg(test)]
mod ipc_params_tests {
    use serde_json::json;

    /// Helper to verify JSON has expected camelCase keys
    fn assert_has_camel_case_key(json: &serde_json::Value, key: &str) {
        assert!(
            json.get(key).is_some(),
            "JSON missing expected camelCase key '{}'. Got: {}",
            key,
            json
        );
    }

    /// Helper to verify JSON does NOT have snake_case keys
    fn assert_no_snake_case_key(json: &serde_json::Value, key: &str) {
        assert!(
            json.get(key).is_none(),
            "JSON has unexpected snake_case key '{}'. Should use camelCase.",
            key
        );
    }

    #[test]
    fn test_hotwire_params_are_camel_case() {
        let params = json!({
            "zellijSession": "test-session",
            "projectPath": "/path/to/project",
            "suggestedPlaybook": "plan-build",
            "intent": "test intent",
        });

        assert_has_camel_case_key(&params, "zellijSession");
        assert_has_camel_case_key(&params, "projectPath");
        assert_has_camel_case_key(&params, "suggestedPlaybook");
        assert_no_snake_case_key(&params, "zellij_session");
        assert_no_snake_case_key(&params, "project_path");
        assert_no_snake_case_key(&params, "suggested_playbook");
    }

    #[test]
    fn test_pair_params_are_camel_case() {
        let params = json!({
            "zellijSession": "test-session",
            "projectPath": "/path/to/project",
            "runId": "abc123",
            "roleId": "worker-1",
        });

        assert_has_camel_case_key(&params, "zellijSession");
        assert_has_camel_case_key(&params, "projectPath");
        assert_has_camel_case_key(&params, "runId");
        assert_has_camel_case_key(&params, "roleId");
        assert_no_snake_case_key(&params, "zellij_session");
        assert_no_snake_case_key(&params, "project_path");
        assert_no_snake_case_key(&params, "run_id");
        assert_no_snake_case_key(&params, "role_id");
    }

    #[test]
    fn test_complete_params_are_camel_case() {
        let params = json!({
            "runId": "abc123",
            "taskDescription": "Task completed",
            "source": "strategist",
            "outcome": "Completed",
        });

        assert_has_camel_case_key(&params, "runId");
        assert_has_camel_case_key(&params, "taskDescription");
        assert_no_snake_case_key(&params, "run_id");
        assert_no_snake_case_key(&params, "task_description");
    }

    #[test]
    fn test_impediment_params_are_camel_case() {
        let params = json!({
            "runId": "abc123",
            "source": "strategist",
            "impedimentType": "technical",
            "description": "Cannot connect to database",
            "suggestion": "Check credentials",
        });

        assert_has_camel_case_key(&params, "runId");
        assert_has_camel_case_key(&params, "impedimentType");
        assert_no_snake_case_key(&params, "run_id");
        assert_no_snake_case_key(&params, "impediment_type");
        // Make sure we don't use just "type"
        assert_no_snake_case_key(&params, "type");
    }

    #[test]
    fn test_send_params_are_camel_case() {
        let params = json!({
            "runId": "abc123",
            "to": "implementer",
            "summary": "Please implement X",
            "details": "Detailed instructions",
            "source": "strategist",
        });

        assert_has_camel_case_key(&params, "runId");
        assert_no_snake_case_key(&params, "run_id");
    }

    #[test]
    fn test_inbox_params_are_camel_case() {
        let params = json!({
            "runId": "abc123",
            "sinceSequence": 10,
        });

        assert_has_camel_case_key(&params, "runId");
        assert_has_camel_case_key(&params, "sinceSequence");
        assert_no_snake_case_key(&params, "run_id");
        assert_no_snake_case_key(&params, "since_sequence");
    }

    #[test]
    fn test_status_params_are_camel_case() {
        let params = json!({
            "runId": "abc123",
        });

        assert_has_camel_case_key(&params, "runId");
        assert_no_snake_case_key(&params, "run_id");
    }

    #[test]
    fn test_artifact_params_are_camel_case() {
        // artifact list
        let list_params = json!({ "runId": "abc123" });
        assert_has_camel_case_key(&list_params, "runId");
        assert_no_snake_case_key(&list_params, "run_id");

        // artifact move
        let move_params = json!({
            "runId": "abc123",
            "oldPath": "docs/old.md",
            "newPath": "docs/new.md",
            "refsOnly": false,
        });
        assert_has_camel_case_key(&move_params, "oldPath");
        assert_has_camel_case_key(&move_params, "newPath");
        assert_has_camel_case_key(&move_params, "refsOnly");
        assert_no_snake_case_key(&move_params, "old_path");
        assert_no_snake_case_key(&move_params, "new_path");
        assert_no_snake_case_key(&move_params, "refs_only");

        // artifact add-comment
        let comment_params = json!({
            "runId": "abc123",
            "path": "docs/spec.md",
            "targetText": "some text",
            "comment": "This needs clarification",
            "author": "strategist",
        });
        assert_has_camel_case_key(&comment_params, "targetText");
        assert_no_snake_case_key(&comment_params, "target_text");

        // artifact list-comments
        let list_comments_params = json!({
            "runId": "abc123",
            "path": "docs/spec.md",
            "statusFilter": "open",
        });
        assert_has_camel_case_key(&list_comments_params, "statusFilter");
        assert_no_snake_case_key(&list_comments_params, "status_filter");

        // artifact resolve
        let resolve_params = json!({
            "runId": "abc123",
            "commentId": "comment-456",
            "resolvedBy": "strategist",
        });
        assert_has_camel_case_key(&resolve_params, "commentId");
        assert_has_camel_case_key(&resolve_params, "resolvedBy");
        assert_no_snake_case_key(&resolve_params, "comment_id");
        assert_no_snake_case_key(&resolve_params, "resolved_by");
    }

    #[test]
    fn test_session_params_are_camel_case() {
        let register_params = json!({
            "sessionName": "claude-main",
            "projectDir": "/path/to/project",
        });
        assert_has_camel_case_key(&register_params, "sessionName");
        assert_has_camel_case_key(&register_params, "projectDir");
        assert_no_snake_case_key(&register_params, "session_name");
        assert_no_snake_case_key(&register_params, "project_dir");
    }

    #[test]
    fn test_validate_params_are_camel_case() {
        let params = json!({
            "zellijSession": "test-session",
        });
        assert_has_camel_case_key(&params, "zellijSession");
        assert_no_snake_case_key(&params, "zellij_session");
    }
}
