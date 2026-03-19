//! Upgrade Hotwired components
//!
//! Updates the Claude Code plugin (prompts, hooks, commands) to the latest version.
//! This ensures agents get the latest playbook commands and lifecycle hooks.

use std::process::Command;

pub fn run() {
    println!("Upgrading Hotwired...");
    println!();

    // Step 1: Update Claude plugin marketplace
    print!("Updating plugin marketplace... ");
    match Command::new("claude")
        .args(["plugin", "marketplace", "add", "hotwired-sh/claude-plugin"])
        .output()
    {
        Ok(output) if output.status.success() => {
            println!("done");
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("already") {
                println!("already registered");
            } else {
                println!("done");
            }
        }
        Err(e) => {
            eprintln!("failed");
            eprintln!("  Could not run 'claude': {}", e);
            eprintln!("  Is Claude Code installed? https://docs.anthropic.com/en/docs/claude-code");
            std::process::exit(1);
        }
    }

    // Step 2: Install/update the plugin
    print!("Installing latest plugin... ");
    match Command::new("claude")
        .args(["plugin", "install", "hotwired@hotwired-sh"])
        .output()
    {
        Ok(output) if output.status.success() => {
            println!("done");
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                println!("  {}", stdout.trim());
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("warning: {}", stderr.trim());
        }
        Err(e) => {
            eprintln!("failed: {}", e);
            std::process::exit(1);
        }
    }

    println!();
    println!("Upgrade complete. Restart Claude Code sessions to pick up new prompts.");
}
