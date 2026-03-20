//! Upgrade the Hotwired CLI and Claude Code commands
//!
//! Updates:
//!   1. The `hotwired` CLI binary (via npm)
//!   2. Claude Code commands (/open, /hotwire, /pair, etc.)
//!
//! Does NOT update the Hotwired desktop app.

use std::process::Command;

const NPM_PACKAGE: &str = "@hotwired-sh/hotwired-cli";

pub fn run() {
    let current_version = env!("CARGO_PKG_VERSION");

    println!("Updating Hotwired CLI and Claude Code commands");
    println!("(Does not update the Hotwired desktop app)");
    println!();

    // Step 1: Update CLI binary
    print!("  CLI (v{current_version}) ... ");
    match check_and_update_cli(current_version) {
        CliUpdate::AlreadyLatest => println!("up to date"),
        CliUpdate::Updated(v) => println!("updated to v{v}"),
        CliUpdate::Failed(msg) => println!("skipped ({msg})"),
    }

    // Step 2: Update Claude Code commands
    print!("  Claude Code commands ... ");
    match update_claude_plugin() {
        PluginUpdate::Updated => println!("updated"),
        PluginUpdate::AlreadyCurrent => println!("up to date"),
        PluginUpdate::Failed(msg) => {
            println!("failed");
            eprintln!("    {msg}");
        }
    }

    println!();
    println!("Done. Restart Claude Code sessions to pick up changes.");
}

enum CliUpdate {
    AlreadyLatest,
    Updated(String),
    Failed(String),
}

fn check_and_update_cli(current_version: &str) -> CliUpdate {
    // Check npm for latest version
    let latest = match Command::new("npm")
        .args(["view", NPM_PACKAGE, "version"])
        .output()
    {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        Ok(_) => return CliUpdate::Failed("could not query npm registry".into()),
        Err(_) => return CliUpdate::Failed("npm not found".into()),
    };

    if latest == current_version {
        return CliUpdate::AlreadyLatest;
    }

    // Install the new version
    let pkg_spec = format!("{NPM_PACKAGE}@{latest}");
    match Command::new("npm")
        .args(["install", "-g", &pkg_spec])
        .output()
    {
        Ok(output) if output.status.success() => CliUpdate::Updated(latest),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            CliUpdate::Failed(format!(
                "npm install failed: {}",
                stderr.lines().next().unwrap_or("")
            ))
        }
        Err(e) => CliUpdate::Failed(format!("npm install failed: {e}")),
    }
}

enum PluginUpdate {
    Updated,
    AlreadyCurrent,
    Failed(String),
}

fn update_claude_plugin() -> PluginUpdate {
    // Register marketplace source (idempotent)
    match Command::new("claude")
        .args(["plugin", "marketplace", "add", "hotwired-sh/claude-plugin"])
        .output()
    {
        Ok(_) => {} // Ignore result, may already be registered
        Err(e) => {
            return PluginUpdate::Failed(format!(
                "Could not run 'claude': {e}. Is Claude Code installed?"
            ));
        }
    }

    // Install/update the plugin
    match Command::new("claude")
        .args(["plugin", "install", "hotwired@hotwired-sh"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("already up to date") || stdout.contains("no changes") {
                PluginUpdate::AlreadyCurrent
            } else {
                PluginUpdate::Updated
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            PluginUpdate::Failed(stderr.trim().to_string())
        }
        Err(e) => PluginUpdate::Failed(format!("Could not run 'claude': {e}")),
    }
}
