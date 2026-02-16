//! Artifact management commands
//!
//! Simplified artifact handling - replaces 14+ MCP tools with 8 CLI commands.
//! See Issue #13 and docs/features/COMMENT_ANCHORING.md for background.

use super::{format_timestamp, handle_error, validate};
use crate::ipc::HotwiredClient;
use std::path::Path;

/// List all tracked artifacts in the current run
pub async fn list(client: &HotwiredClient) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_list",
            serde_json::json!({ "runId": state.run_id }),
        )
        .await
    {
        Ok(response) if response.success => {
            let artifacts = response
                .data
                .as_ref()
                .and_then(|d| d.get("artifacts"))
                .and_then(|a| a.as_array())
                .cloned()
                .unwrap_or_default();

            if artifacts.is_empty() {
                println!("No tracked artifacts.");
                return;
            }

            println!(
                "{:<30} {:<8} {:<8} {:<8} {:<20}",
                "PATH", "STATUS", "COMMENTS", "VERSIONS", "TITLE"
            );
            for a in &artifacts {
                let path = a.get("path").and_then(|v| v.as_str()).unwrap_or("-");
                let status = a.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                let comments = a.get("commentCount").and_then(|v| v.as_i64()).unwrap_or(0);
                let versions = a.get("versionCount").and_then(|v| v.as_i64()).unwrap_or(0);
                let title = a.get("title").and_then(|v| v.as_str()).unwrap_or("-");

                // Truncate title if too long
                let title_display = if title.len() > 20 {
                    format!("{}...", &title[..17])
                } else {
                    title.to_string()
                };

                // Highlight missing status
                let status_display = match status {
                    "ok" => "ok",
                    "missing" => "MISSING",
                    _ => status,
                };

                println!(
                    "{:<30} {:<8} {:<8} {:<8} {:<20}",
                    path, status_display, comments, versions, title_display
                );
            }
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "unknown".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

/// Sync a file (register new or update existing)
pub async fn sync(client: &HotwiredClient, path: &Path) {
    let state = validate::require_session(client).await;

    // Check file exists
    if !path.exists() {
        eprintln!("error: file not found: {}", path.display());
        std::process::exit(1);
    }

    match client
        .request(
            "artifact_sync",
            serde_json::json!({
                "runId": state.run_id,
                "path": path.to_string_lossy(),
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();
            let status = data
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let title = data
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled");
            let version = data.get("version").and_then(|v| v.as_i64()).unwrap_or(1);
            let relocated = data
                .get("commentsRelocated")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let orphaned = data
                .get("commentsOrphaned")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            match status {
                "registered" => {
                    println!("Artifact registered: {}", path.display());
                    println!("  Title: {}", title);
                    println!("  Version: {}", version);
                }
                "synced" => {
                    println!("Artifact synced: {}", path.display());
                    println!("  Title: {}", title);
                    println!("  Version: {}", version);
                    if relocated > 0 || orphaned > 0 {
                        println!("  {} comments relocated, {} orphaned", relocated, orphaned);
                    }
                }
                _ => println!("Status: {}", status),
            }
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "unknown".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

/// Move an artifact to a new path (preserves comments)
pub async fn move_artifact(
    client: &HotwiredClient,
    old_path: &Path,
    new_path: &Path,
    refs_only: bool,
) {
    let state = validate::require_session(client).await;

    // Validation
    if refs_only {
        // refs_only mode: new file must exist
        if !new_path.exists() {
            eprintln!("error: new file not found: {}", new_path.display());
            eprintln!("When using --refs-only, the file must already exist at the new location.");
            std::process::exit(1);
        }
    } else {
        // Normal mode: old file must exist
        if !old_path.exists() {
            eprintln!("error: source file not found: {}", old_path.display());
            eprintln!("Use --refs-only if the file was already moved.");
            std::process::exit(1);
        }
    }

    match client
        .request(
            "artifact_move",
            serde_json::json!({
                "runId": state.run_id,
                "oldPath": old_path.to_string_lossy(),
                "newPath": new_path.to_string_lossy(),
                "refsOnly": refs_only,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();
            let preserved = data
                .get("commentsPreserved")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let file_moved = data
                .get("fileMoved")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if file_moved {
                println!(
                    "File moved: {} → {}",
                    old_path.display(),
                    new_path.display()
                );
            }
            println!(
                "Artifact refs updated: {} → {}",
                old_path.display(),
                new_path.display()
            );
            println!("  {} comments preserved", preserved);
        }
        Ok(response) => {
            // Backend should return specific errors:
            // - "artifact_not_found" if old_path not in artifacts table
            // - "file_not_found" if refs_only but new_path doesn't exist
            let err = response.error.unwrap_or_else(|| "unknown".into());
            if err.contains("not found") || err.contains("not tracked") {
                eprintln!("error: {}", err);
                eprintln!();
                eprintln!("The artifact must be synced first. Run:");
                eprintln!("  hotwired-cli artifact sync {}", old_path.display());
            } else {
                eprintln!("error: {}", err);
            }
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

/// Add a comment anchored to specific text
pub async fn add_comment(client: &HotwiredClient, path: &Path, target_text: &str, message: &str) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_add_comment",
            serde_json::json!({
                "runId": state.run_id,
                "path": path.to_string_lossy(),
                "targetText": target_text,
                "comment": message,
                "author": state.role_id,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();
            let comment_id = data
                .get("commentId")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!("Comment added: {}", comment_id);
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "unknown".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

/// List comments on an artifact
pub async fn list_comments(client: &HotwiredClient, path: &Path, status_filter: &str) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_list_comments",
            serde_json::json!({
                "runId": state.run_id,
                "path": path.to_string_lossy(),
                "statusFilter": status_filter,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let comments = response
                .data
                .as_ref()
                .and_then(|d| d.get("comments"))
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();

            if comments.is_empty() {
                println!("No comments.");
                return;
            }

            for c in &comments {
                let id = c.get("commentId").and_then(|v| v.as_str()).unwrap_or("?");
                let target = c.get("targetText").and_then(|v| v.as_str()).unwrap_or("");
                let msg = c.get("comment").and_then(|v| v.as_str()).unwrap_or("");
                let status = c.get("status").and_then(|v| v.as_str()).unwrap_or("?");

                let target_preview = if target.len() > 30 {
                    format!("{}...", &target[..30])
                } else {
                    target.to_string()
                };

                println!("[{}] \"{}\" - {} ({})", id, target_preview, msg, status);
            }
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "unknown".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

/// Resolve a comment
pub async fn resolve(client: &HotwiredClient, comment_id: &str) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_resolve_comment",
            serde_json::json!({
                "runId": state.run_id,
                "commentId": comment_id,
                "resolvedBy": state.role_id,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            println!("Comment resolved: {}", comment_id);
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "unknown".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

/// List all versions of an artifact
pub async fn list_versions(client: &HotwiredClient, path: &Path) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_list_versions",
            serde_json::json!({
                "runId": state.run_id,
                "path": path.to_string_lossy(),
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let versions = response
                .data
                .as_ref()
                .and_then(|d| d.get("versions"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if versions.is_empty() {
                println!("No versions found. Run `artifact sync` first.");
                return;
            }

            println!("{:<8} {:<20} CHANGES", "VERSION", "TIMESTAMP");
            for v in &versions {
                let version = v.get("version").and_then(|x| x.as_i64()).unwrap_or(0);
                let timestamp = v.get("timestamp").and_then(|x| x.as_str()).unwrap_or("-");
                let added = v.get("linesAdded").and_then(|x| x.as_i64()).unwrap_or(0);
                let removed = v.get("linesRemoved").and_then(|x| x.as_i64()).unwrap_or(0);

                let changes = if version == 1 {
                    "(initial)".to_string()
                } else {
                    format!("+{} -{} lines", added, removed)
                };

                println!(
                    "{:<8} {:<20} {}",
                    version,
                    format_timestamp(timestamp),
                    changes
                );
            }
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "unknown".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

/// Show content of a specific version
pub async fn get_version(client: &HotwiredClient, path: &Path, version: u32) {
    let state = validate::require_session(client).await;

    match client
        .request(
            "artifact_get_version",
            serde_json::json!({
                "runId": state.run_id,
                "path": path.to_string_lossy(),
                "version": version,
            }),
        )
        .await
    {
        Ok(response) if response.success => {
            let data = response.data.unwrap_or_default();
            let title = data
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled");
            let timestamp = data
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("-");
            let content = data.get("content").and_then(|v| v.as_str()).unwrap_or("");

            println!("# {} (version {})", title, version);
            println!("# Synced: {}", format_timestamp(timestamp));
            println!("# {}", "-".repeat(60));
            println!();
            println!("{}", content);
        }
        Ok(response) => {
            eprintln!(
                "error: {}",
                response.error.unwrap_or_else(|| "unknown".into())
            );
            std::process::exit(1);
        }
        Err(e) => handle_error(e),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_title_truncation() {
        let long_title = "This is a very long title that should be truncated";
        let truncated = if long_title.len() > 20 {
            format!("{}...", &long_title[..17])
        } else {
            long_title.to_string()
        };
        assert_eq!(truncated.len(), 20);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_status_display() {
        let statuses = vec![("ok", "ok"), ("missing", "MISSING"), ("unknown", "unknown")];
        for (input, expected) in statuses {
            let display = match input {
                "ok" => "ok",
                "missing" => "MISSING",
                _ => input,
            };
            assert_eq!(display, expected);
        }
    }

    #[test]
    fn test_target_text_preview() {
        let long_text = "This is some very long target text that needs to be truncated for display";
        let preview = if long_text.len() > 30 {
            format!("{}...", &long_text[..30])
        } else {
            long_text.to_string()
        };
        assert!(preview.len() <= 33); // 30 + "..."
    }
}
