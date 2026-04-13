use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum AdminCommand {
    /// Remove runtime state for all workflows and clear runtime data directories.
    #[command(after_help = "\
Deletes all workflow data: workspace, transcripts, artifacts, blueprints,
and their database records. The --yes flag is required to execute.

Examples:
  palette admin reset --dry-run   # Preview what would be deleted
  palette admin reset --yes       # Delete everything")]
    Reset(ResetArgs),
    /// Garbage-collect stale workflows and their runtime artifacts.
    #[command(after_help = "\
By default, targets completed and failed workflows. Use filters to narrow
the scope. The --yes flag is required to execute.

Examples:
  palette admin gc --dry-run                    # Preview default targets
  palette admin gc --older-than-hours 168 --yes # Delete workflows older than 7 days
  palette admin gc --workflow-id abc123 --yes   # Delete a specific workflow")]
    Gc(GcArgs),
}

#[derive(Args)]
pub struct ResetArgs {
    /// Path to the configuration file (overrides the default user config)
    #[arg(short, long)]
    pub config: Option<String>,
    /// Preview what would be deleted without deleting anything
    #[arg(long)]
    pub dry_run: bool,
    /// Confirm destructive operation
    #[arg(long)]
    pub yes: bool,
}

#[derive(Args)]
pub struct GcArgs {
    /// Path to the configuration file (overrides the default user config)
    #[arg(short, long)]
    pub config: Option<String>,
    /// Target specific workflow IDs (repeatable)
    #[arg(long = "workflow-id")]
    pub workflow_ids: Vec<String>,
    /// Include active/suspending workflows in candidates
    #[arg(long)]
    pub include_active: bool,
    /// Keep only workflows older than this number of hours
    #[arg(long)]
    pub older_than_hours: Option<i64>,
    /// Preview what would be deleted without deleting anything
    #[arg(long)]
    pub dry_run: bool,
    /// Confirm destructive operation
    #[arg(long)]
    pub yes: bool,
}
