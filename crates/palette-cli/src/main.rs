mod admin;
mod config;
mod doctor;
mod interactor_factory;
mod start;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "palette", about = "Autonomous AI agent orchestration system")]
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
    Doctor,
    /// Administrative maintenance commands
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
        Some(Command::Start { config }) => start::run(config.as_deref()).await,
        None => start::run(None).await,
    }
}
