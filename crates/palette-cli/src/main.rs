mod admin;
mod config;
mod doctor;
mod interactor_factory;
mod shutdown;
mod start;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "palette",
    about = "Autonomous AI agent orchestration system",
    after_help = "Run without a subcommand to start the Orchestrator (equivalent to `palette start`)."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the Orchestrator
    Start {
        /// Path to the configuration file (overrides the default user config)
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Check prerequisites and system health (outputs JSON)
    #[command(long_about = "\
Check prerequisites and system health (outputs JSON).

Checks:
  - Rust toolchain (cargo)
  - Docker daemon
  - tmux
  - git
  - GitHub CLI authentication (gh auth status)
  - Worker Docker images (palette-base, palette-worker)")]
    Doctor,
    /// Gracefully shut down a running Orchestrator
    Shutdown {
        /// Path to the configuration file (overrides the default user config)
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Administrative maintenance commands
    #[command(after_help = "These commands should be run while the Orchestrator is stopped.")]
    Admin(AdminArgs),
}

#[derive(Args)]
struct AdminArgs {
    #[command(subcommand)]
    command: admin::AdminCommand,
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    init_tracing();

    match cli.command {
        Some(Command::Doctor) => {
            if !doctor::run().await? {
                std::process::exit(1);
            }
            Ok(())
        }
        Some(Command::Admin(args)) => admin::run(args.command),
        Some(Command::Shutdown { config }) => shutdown::run(config.as_deref()).await,
        Some(Command::Start { config }) => start::run(config.as_deref()).await,
        None => start::run(None).await,
    }
}
