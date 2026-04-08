mod config;
mod doctor;
mod start;

use clap::{Parser, Subcommand};

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
        #[arg(short, long, default_value = "config/palette.toml")]
        config: String,
    },
    /// Check prerequisites and system health
    Doctor {
        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Doctor { json }) => {
            doctor::run(json).await;
            Ok(())
        }
        Some(Command::Start { config }) => start::run(&config).await,
        None => start::run("config/palette.toml").await,
    }
}
