mod config;
mod doctor;
mod start;

use clap::{Parser, Subcommand};

const DEFAULT_CONFIG_PATH: &str = "config/palette.toml";

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
        /// Path to the configuration file
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: String,
    },
    /// Check prerequisites and system health (outputs JSON)
    Doctor,
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
        Some(Command::Start { config }) => start::run(&config).await,
        None => start::run(DEFAULT_CONFIG_PATH).await,
    }
}
