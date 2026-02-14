mod commands;
mod ipc;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    // =========================================================================
    // MANAGEMENT COMMANDS
    // =========================================================================

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

    // =========================================================================
    // WORKFLOW COMMANDS (for agents)
    // =========================================================================

    /// Start a new workflow run
    ///
    /// Initializes a new Hotwired workflow. The terminal becomes attached
    /// to the run and receives the protocol instructions.
    ///
    /// Examples:
    ///   hotwired-cli hotwire --intent "Build user authentication"
    ///   hotwired-cli hotwire --playbook architect-team --intent "Implement OAuth"
    Hotwire {
        /// Playbook to use (e.g., plan-build, architect-team)
        #[arg(long)]
        playbook: Option<String>,

        /// What you want to accomplish
        #[arg(long)]
        intent: Option<String>,

        /// Project directory (defaults to current dir)
        #[arg(long)]
        project: Option<PathBuf>,
    },

    /// Join an existing workflow run
    ///
    /// Attaches this terminal to an existing run. You'll receive the
    /// protocol instructions for your assigned role.
    ///
    /// Examples:
    ///   hotwired-cli pair abc123
    ///   hotwired-cli pair abc123 --role worker-1
    Pair {
        /// Run ID to join
        run_id: String,

        /// Role to take (e.g., worker-1, builder)
        #[arg(long)]
        role: Option<String>,
    },

    /// Send a message to another participant
    ///
    /// Sends a handoff or message to another agent or the human operator.
    ///
    /// Examples:
    ///   hotwired-cli send --to orchestrator "Task 1.1 complete"
    ///   hotwired-cli send --to human "Need clarification on auth approach"
    Send {
        /// Recipient: orchestrator, implementer, human, or role ID
        #[arg(long)]
        to: String,

        /// Message content
        #[arg(trailing_var_arg = true)]
        message: Vec<String>,
    },

    /// Check for incoming messages
    ///
    /// Retrieves recent messages from the conversation.
    ///
    /// Examples:
    ///   hotwired-cli inbox
    ///   hotwired-cli inbox --watch
    ///   hotwired-cli inbox --since 42
    Inbox {
        /// Continuously watch for new messages
        #[arg(long)]
        watch: bool,

        /// Only show messages after this sequence number
        #[arg(long)]
        since: Option<i64>,
    },

    /// Mark the current task as complete
    ///
    /// Signals that your assigned work is done.
    ///
    /// Examples:
    ///   hotwired-cli complete
    ///   hotwired-cli complete --outcome "All tests passing"
    Complete {
        /// Description of the outcome
        #[arg(long)]
        outcome: Option<String>,
    },

    /// Report a blocker/impediment
    ///
    /// Signals that you're stuck and need help.
    ///
    /// Examples:
    ///   hotwired-cli impediment "Cannot access database"
    ///   hotwired-cli impediment "Need push access" --type access --suggestion "Grant write perms"
    Impediment {
        /// Description of the blocker
        description: String,

        /// Type: technical, access, clarification, decision
        #[arg(long, default_value = "technical")]
        r#type: String,

        /// Suggested resolution
        #[arg(long)]
        suggestion: Option<String>,
    },

    /// Check current run status
    ///
    /// Shows the status of the attached run and connected agents.
    ///
    /// Example output:
    ///
    ///   Run:      abc123
    ///   Status:   active
    ///   Phase:    executing
    ///   Playbook: architect-team
    ///   My Role:  worker-1 (active)
    ///
    ///   Connected Agents:
    ///     - architect (me) - active
    ///     - worker-1 - awaiting_response
    Status,

    // =========================================================================
    // ARTIFACT COMMANDS
    // =========================================================================

    /// Manage document artifacts
    ///
    /// Track documents, add comments, and view version history.
    /// Replaces the complex MCP artifact tools with simple file-based workflow.
    ///
    /// Examples:
    ///   hotwired-cli artifact ls
    ///   hotwired-cli artifact sync docs/PRD.md
    ///   hotwired-cli artifact comment docs/PRD.md "auth flow" "Consider OAuth"
    Artifact {
        #[command(subcommand)]
        action: ArtifactAction,
    },
}

#[derive(Subcommand)]
enum ArtifactAction {
    /// List all tracked artifacts
    ///
    /// Shows path, status (ok/MISSING), comment count, version count, and title.
    ///
    /// Example output:
    ///
    ///   PATH                           STATUS   COMMENTS VERSIONS TITLE
    ///   docs/PRD.md                    ok       3        5        Product Requirements
    ///   docs/DESIGN.md                 MISSING  1        2        System Design
    #[command(alias = "ls")]
    List,

    /// Sync a file (register new or update existing)
    ///
    /// Registers a new artifact or updates an existing one.
    /// Creates a versioned snapshot and relocates comment anchors.
    ///
    /// Examples:
    ///   hotwired-cli artifact sync docs/PRD.md
    Sync {
        /// Path to the file
        path: PathBuf,
    },

    /// Move an artifact to a new path (preserves comments)
    ///
    /// By default, MOVES the file on disk AND updates artifact refs.
    /// Use --refs-only if the file was already moved and you just need to update refs.
    ///
    /// NOTE: The artifact must already be synced. If not, run `artifact sync` first.
    ///
    /// Examples:
    ///   hotwired-cli artifact mv docs/old.md docs/new.md
    ///   hotwired-cli artifact mv docs/old.md docs/new.md --refs-only
    #[command(alias = "mv")]
    Move {
        /// Current path (where artifact is registered)
        old_path: PathBuf,
        /// New path
        new_path: PathBuf,
        /// Only update refs, don't move the file (use when file already moved)
        #[arg(long)]
        refs_only: bool,
    },

    /// Add a comment anchored to specific text
    ///
    /// Comments are anchored to the target text content, not line numbers.
    /// They will be relocated automatically when the file is edited and synced.
    ///
    /// Examples:
    ///   hotwired-cli artifact comment docs/PRD.md "authentication flow" "Consider OAuth2"
    Comment {
        /// Path to the artifact
        path: PathBuf,
        /// Text to anchor the comment to (must exist in the document)
        target_text: String,
        /// Comment message
        message: String,
    },

    /// List comments on an artifact
    ///
    /// Example output:
    ///
    ///   [cmt_abc123] "authentication flow..." - Consider OAuth2 (open)
    ///   [cmt_def456] "rate limiting..." - Add to MVP scope? (resolved)
    Comments {
        /// Path to the artifact
        path: PathBuf,
        /// Filter by status: open, resolved, all
        #[arg(long, default_value = "open")]
        status: String,
    },

    /// Resolve a comment
    ///
    /// Marks a comment as resolved.
    ///
    /// Examples:
    ///   hotwired-cli artifact resolve cmt_abc123
    Resolve {
        /// Comment ID
        comment_id: String,
    },

    /// List all versions of an artifact
    ///
    /// Shows version history with timestamps and change stats.
    /// Useful for debugging agent changes over time.
    ///
    /// Example output:
    ///
    ///   VERSION  TIMESTAMP            CHANGES
    ///   3        2024-01-15 14:30:00  +50 -12 lines
    ///   2        2024-01-15 13:15:00  +120 -5 lines
    ///   1        2024-01-15 10:00:00  (initial)
    Versions {
        /// Path to the artifact
        path: PathBuf,
    },

    /// Show content of a specific version
    ///
    /// Retrieves the full document content at a specific version.
    ///
    /// Examples:
    ///   hotwired-cli artifact version docs/PRD.md 2
    Version {
        /// Path to the artifact
        path: PathBuf,
        /// Version number
        version: u32,
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

    let client = ipc::HotwiredClient::new(args.socket_path);

    match args.command {
        // Management commands
        Some(Commands::Run { action }) => match action {
            RunAction::List => commands::run::list(&client).await,
            RunAction::Show { id } => commands::run::show(&client, &id).await,
            RunAction::Remove { id } => commands::run::remove(&client, &id).await,
        },
        Some(Commands::Session { action }) => match action {
            SessionAction::List => commands::session::list(&client).await,
            SessionAction::Show { name } => commands::session::show(&client, &name).await,
            SessionAction::Remove { name } => commands::session::remove(&client, &name).await,
        },
        Some(Commands::Auth { action }) => match action {
            AuthAction::Status => commands::auth::status(&client).await,
        },

        // Workflow commands
        Some(Commands::Hotwire { playbook, intent, project }) => {
            commands::hotwire::run(&client, playbook, intent, project).await;
        }
        Some(Commands::Pair { run_id, role }) => {
            commands::pair::run(&client, &run_id, role.as_deref()).await;
        }
        Some(Commands::Send { to, message }) => {
            let msg = message.join(" ");
            commands::send::run(&client, &to, &msg).await;
        }
        Some(Commands::Inbox { watch, since }) => {
            commands::inbox::run(&client, watch, since).await;
        }
        Some(Commands::Complete { outcome }) => {
            commands::complete::run(&client, outcome).await;
        }
        Some(Commands::Impediment { description, r#type, suggestion }) => {
            commands::impediment::run(&client, &description, &r#type, suggestion).await;
        }
        Some(Commands::Status) => {
            commands::status::run(&client).await;
        }

        // Artifact commands
        Some(Commands::Artifact { action }) => match action {
            ArtifactAction::List => commands::artifact::list(&client).await,
            ArtifactAction::Sync { path } => commands::artifact::sync(&client, &path).await,
            ArtifactAction::Move { old_path, new_path, refs_only } => {
                commands::artifact::move_artifact(&client, &old_path, &new_path, refs_only).await;
            }
            ArtifactAction::Comment { path, target_text, message } => {
                commands::artifact::add_comment(&client, &path, &target_text, &message).await;
            }
            ArtifactAction::Comments { path, status } => {
                commands::artifact::list_comments(&client, &path, &status).await;
            }
            ArtifactAction::Resolve { comment_id } => {
                commands::artifact::resolve(&client, &comment_id).await;
            }
            ArtifactAction::Versions { path } => {
                commands::artifact::list_versions(&client, &path).await;
            }
            ArtifactAction::Version { path, version } => {
                commands::artifact::get_version(&client, &path, version).await;
            }
        },

        None => {
            use clap::CommandFactory;
            Args::command().print_help()?;
            println!();
        }
    }

    Ok(())
}
