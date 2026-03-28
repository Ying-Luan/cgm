//! CLI module entry, defines command line interface and subcommands
//!
//! Supports start, stop, submit, status and other subcommands.

mod cancel;
mod delete;
mod list;
mod log;
mod start;
mod status;
mod stop;
mod submit;

use clap::{Parser, Subcommand};

use crate::types::SchedulerKind;

/// CLI struct
#[derive(Parser)]
#[command(
    name = "cgm",
    version = "0.1.0",
    about = "Convenient GPU Manager Daemon",
    arg_required_else_help = true
)]
struct Cli {
    /// Subcommand
    #[command(subcommand)]
    command: Commands,
}

/// CLI subcommand enum
#[derive(Subcommand)]
enum Commands {
    /// Start daemon
    Start {
        /// Force start. Recreates database. Used when database is corrupted
        #[arg(
            short,
            long,
            default_value = "false",
            help = "Force start. Recreates database. Used when database is corrupted"
        )]
        force: bool,
        /// GPU list to manage, comma-separated or all for all GPUs
        #[arg(
            short,
            long,
            default_value = "all",
            help = "GPU list to manage, comma-separated (e.g. 0,1,2,3) or all for all GPUs"
        )]
        gpus: String,
        /// Scheduling interval in seconds
        #[arg(
            short,
            long,
            default_value = "10",
            help = "Scheduling interval in seconds"
        )]
        interval: u32,
        /// Scheduler strategy. Options: greedy, fifo
        #[arg(
            short,
            long,
            default_value = "greedy",
            help = "Scheduler strategy. Options: greedy, fifo"
        )]
        scheduler: SchedulerKind,
        /// GPU memory threshold (%), above this value considered externally occupied
        #[arg(
            short,
            long,
            default_value = "10",
            help = "GPU memory threshold (%), above this value considered externally occupied"
        )]
        threshold: u32,
    },
    /// Stop daemon
    Stop {
        /// Force stop. Shuts down even if jobs are running
        #[arg(
            short,
            long,
            default_value = "false",
            help = "Force stop. Shuts down even if jobs are running"
        )]
        force: bool,
    },
    /// Submit job
    Submit {
        /// Detach mode. Opens less to follow log after submission
        #[arg(short, long, default_value = "false", help = "Detach mode")]
        detach: bool,
        /// Number of GPUs to request
        #[arg(short, long, default_value = "1", help = "Number of GPUs to request")]
        gpus: usize,
        /// Log file path
        #[arg(short, long, help = "Log file path")]
        log: Option<String>,
        /// Command to execute
        #[arg(last = true, required = true, help = "Command to execute")]
        command: Vec<String>,
    },
    /// Cancel job
    Cancel {
        /// Job ID
        #[arg(help = "Job ID")]
        id: usize,
        /// Force cancel running job
        #[arg(
            short,
            long,
            default_value = "false",
            help = "Force cancel running job"
        )]
        force: bool,
    },
    /// Delete job
    Delete {
        /// Job ID to delete
        #[arg(help = "Job ID to delete", conflicts_with_all = ["all", "status"], required_unless_present_any = ["all", "status"])]
        id: Option<usize>,
        /// Delete all terminated jobs (completed/failed/cancelled)
        #[arg(
            short,
            long,
            default_value = "false",
            help = "Delete all terminated jobs (completed/failed/cancelled)",
            conflicts_with_all = ["id", "status"],
        )]
        all: bool,
        /// Delete by status, comma-separated. Values: completed, failed, cancelled
        #[arg(short, long, help = "Delete by status, comma-separated. Values: completed, failed, cancelled", conflicts_with_all = ["id", "all"])]
        status: Option<String>,
    },
    /// View status
    Status,
    /// View job list
    List {
        /// Show all jobs without limit
        #[arg(
            short,
            long,
            default_value = "false",
            help = "Show all jobs without limit",
            conflicts_with = "limit"
        )]
        all: bool,
        /// Show the latest N jobs
        #[arg(
            short,
            long,
            default_value = "20",
            help = "Show the latest N jobs",
            conflicts_with = "all"
        )]
        limit: usize,
    },
    /// View job log
    Log {
        /// Job ID
        #[arg(help = "Job ID")]
        id: usize,
    },
}

/// Parse command line and execute subcommands
pub fn run() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            force,
            gpus,
            interval,
            scheduler,
            threshold,
        } => start::run(force, gpus, interval, scheduler, threshold),
        Commands::Stop { force } => stop::run(force),
        Commands::Submit {
            detach,
            gpus,
            log,
            command,
        } => submit::run(detach, gpus, log, command),
        Commands::Cancel { id, force } => cancel::run(id, force),
        Commands::Delete { id, all, status } => delete::run(id, all, status),
        Commands::Status => status::run(),
        Commands::List { all, limit } => list::run(all, limit),
        Commands::Log { id } => log::run(id),
    }
}
