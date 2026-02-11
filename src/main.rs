mod commands;
mod ipc;

use clap::{Parser, Subcommand};

/// Hotwired CLI - manage workflows, sessions, and runs
#[derive(Parser)]
#[command(name = "hotwired-cli")]
#[command(about = "CLI for Hotwired multi-agent workflow orchestration")]
#[command(disable_version_flag = true)]
struct Args {
    /// Print version information (cli and backend)
    #[arg(long, short = 'V')]
    version: bool,

    /// Path to the Unix socket for communicating with the Hotwired backend.
    /// Defaults to ~/.hotwired/hotwired.sock
    #[arg(long, short = 's', global = true)]
    socket_path: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage workflow runs
    ///
    /// List, inspect, and remove workflow runs.
    ///
    /// Examples:
    ///   hotwired-cli run list
    ///   hotwired-cli run ls
    ///   hotwired-cli run show a1b2c3d4
    ///   hotwired-cli run rm a1b2c3d4
    Run {
        #[command(subcommand)]
        action: RunAction,
    },

    /// Manage agent sessions
    ///
    /// List, inspect, and remove active agent sessions.
    ///
    /// Examples:
    ///   hotwired-cli session list
    ///   hotwired-cli session ls
    ///   hotwired-cli session show hotwired-strategist
    ///   hotwired-cli session rm hotwired-builder
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Authentication and connection status
    ///
    /// Check if the Hotwired backend is running and whether
    /// the current user has a valid auth token configured.
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
}

#[derive(Subcommand)]
enum RunAction {
    /// List all runs
    ///
    /// Example output:
    ///
    ///   ID         STATUS       PHASE          PLAYBOOK                 CREATED
    ///   a1b2c3d4   active       executing      Plan > Build             2024-01-15 10:30:00
    ///   e5f6g7h8   completed    complete       Solo Build               2024-01-14 09:15:00
    #[command(alias = "ls")]
    List,

    /// Show details of a run
    ///
    /// Accepts full UUIDs or short prefixes (like git).
    ///
    /// Example output:
    ///
    ///   Run:        a1b2c3d4-e5f6-7890-abcd-ef1234567890
    ///   Status:     active
    ///   Phase:      executing
    ///   Playbook:   Plan > Build
    ///   Protocol:   yes
    ///
    ///   Agents:
    ///     strategist       hotwired-strategist          (claude)
    ///     builder          hotwired-builder             (gemini)
    Show {
        /// Run ID (full UUID or short prefix)
        id: String,
    },

    /// Remove a run and its associated data
    ///
    /// Accepts full UUIDs or short prefixes (like git).
    #[command(alias = "rm")]
    Remove {
        /// Run ID (full UUID or short prefix)
        id: String,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// List active agent sessions
    ///
    /// Example output:
    ///
    ///   SESSION                      PROJECT                                      WORKTREE
    ///   hotwired-strategist          /Users/dev/Code/my-project                   no
    ///   hotwired-builder             /Users/dev/Code/my-project                   yes
    #[command(alias = "ls")]
    List,

    /// Show details of a session
    ///
    /// Example output:
    ///
    ///   Session:    hotwired-strategist
    ///   Project:    /Users/dev/Code/my-project
    ///   Worktree:   no
    Show {
        /// Session name
        name: String,
    },

    /// Remove (deregister) an active session
    #[command(alias = "rm")]
    Remove {
        /// Session name
        name: String,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Show connection and authentication status
    ///
    /// Checks whether the Hotwired backend (hotwired-core) is reachable
    /// via the Unix socket and whether an auth token is configured.
    ///
    /// Example output when connected:
    ///
    ///   Backend:    running (v0.1.0)
    ///   Socket:     ~/.hotwired/hotwired.sock
    ///   Auth token: configured
    ///
    /// Example output when disconnected:
    ///
    ///   Backend:    not running
    ///   Socket:     ~/.hotwired/hotwired.sock (not found)
    ///   Auth token: not configured
    Status,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.version {
        commands::print_version(args.socket_path).await;
        return Ok(());
    }

    match args.command {
        Some(Commands::Run { action }) => {
            let client = ipc::HotwiredClient::new(args.socket_path);
            match action {
                RunAction::List => commands::run::list(&client).await,
                RunAction::Show { id } => commands::run::show(&client, &id).await,
                RunAction::Remove { id } => commands::run::remove(&client, &id).await,
            }
        }
        Some(Commands::Session { action }) => {
            let client = ipc::HotwiredClient::new(args.socket_path);
            match action {
                SessionAction::List => commands::session::list(&client).await,
                SessionAction::Show { name } => commands::session::show(&client, &name).await,
                SessionAction::Remove { name } => {
                    commands::session::remove(&client, &name).await
                }
            }
        }
        Some(Commands::Auth { action }) => {
            let client = ipc::HotwiredClient::new(args.socket_path);
            match action {
                AuthAction::Status => commands::auth::status(&client).await,
            }
        }
        None => {
            use clap::CommandFactory;
            Args::command().print_help()?;
            println!();
        }
    }

    Ok(())
}
