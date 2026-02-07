mod ipc;

use clap::Parser;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Hotwired CLI - manage workflows, sessions, and runs
#[derive(Parser)]
#[command(name = "hotwired-cli")]
#[command(about = "CLI for Hotwired multi-agent workflow orchestration")]
#[command(version = VERSION)]
struct Args {
    /// Print version information including hotwired-core version
    #[arg(long = "version-all", short = 'V')]
    version_all: bool,

    /// Path to the Unix socket for communicating with the Hotwired backend.
    /// Defaults to ~/.hotwired/hotwired.sock
    #[arg(long, short = 's', global = true)]
    socket_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.version_all {
        print_version_all(args.socket_path).await;
        return Ok(());
    }

    // No command provided yet - print help
    use clap::CommandFactory;
    Args::command().print_help()?;
    println!();

    Ok(())
}

async fn print_version_all(socket_path: Option<String>) {
    println!("hotwired-cli {}", VERSION);

    let client = ipc::HotwiredClient::new(socket_path);
    match client.health_check().await {
        Ok(response) if response.success => {
            if let Some(data) = response.data {
                if let Some(version) = data.get("version").and_then(|v| v.as_str()) {
                    println!("hotwired-core {}", version);
                } else {
                    println!("hotwired-core connected (version unknown)");
                }
            } else {
                println!("hotwired-core connected (version unknown)");
            }
        }
        Ok(_) => {
            println!("hotwired-core not responding");
        }
        Err(e) => {
            println!("hotwired-core {}", e);
        }
    }
}
