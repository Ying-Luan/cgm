//! CLI module entry, defines command line interface and subcommands
//!
//! Supports start, stop, submit, status and other subcommands.

mod cancel;
mod config;
mod delete;
mod list;
mod log;
mod rerun;
mod start;
mod status;
mod stop;
mod submit;
mod utils;

use clap::{Parser, Subcommand};

use crate::types::SchedulerKind;

/// CLI struct
#[derive(Parser)]
#[command(
    name = "cgm",
    version = "0.1.0",
    about = "Convenient GPU Manager",
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
            help = "GPU list to manage, comma-separated (e.g. 0,1,2,3) or all for all GPUs; defaults to config or all"
        )]
        gpus: Option<String>,
        /// Scheduling interval in seconds
        #[arg(
            short,
            long,
            help = "Scheduling interval in seconds; defaults to config or 10"
        )]
        interval: Option<u32>,
        /// Scheduler strategy. Options: greedy, fifo
        #[arg(
            short,
            long,
            help = "Scheduler strategy. Options: greedy, fifo; defaults to config or greedy"
        )]
        scheduler: Option<SchedulerKind>,
        /// GPU memory threshold (%), above this value considered externally occupied
        #[arg(
            short,
            long,
            help = "GPU memory threshold (%), above this value considered externally occupied; defaults to config or 10"
        )]
        threshold: Option<u32>,
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
        #[arg(
            short,
            long,
            conflicts_with = "follow",
            help = "Detach mode. Opens less to follow log after submission"
        )]
        detach: bool,
        /// Follow mode. Opens less to follow the job log after submission
        #[arg(
            short,
            long,
            conflicts_with = "detach",
            help = "Follow mode. Opens less to follow the job log after submission"
        )]
        follow: bool,
        /// Number of GPUs to request
        #[arg(
            short,
            long,
            help = "Number of GPUs to request; defaults to config or 1"
        )]
        gpus: Option<usize>,
        /// Log file path
        #[arg(short, long, help = "Log file path")]
        log: Option<String>,
        /// Command to execute
        #[arg(last = true, required = true, help = "Command to execute")]
        command: Vec<String>,
    },
    /// Rerun an existing job as a new job
    Rerun {
        /// Source job ID
        #[arg(help = "Source job ID")]
        id: usize,
        /// Use current environment instead of saved environment
        #[arg(
            short = 'e',
            long,
            default_value = "false",
            help = "Use current environment instead of saved environment"
        )]
        current_env: bool,
        /// Enable detach mode. Do not open the log viewer after submission
        #[arg(
            short,
            long,
            conflicts_with = "follow",
            help = "Enable detach mode. Do not open the log viewer after submission"
        )]
        detach: bool,
        /// Open less to follow the new job log
        #[arg(
            short,
            long,
            conflicts_with = "detach",
            help = "Open less to follow the new job log"
        )]
        follow: bool,
        /// Override number of GPUs
        #[arg(short, long, help = "Override number of GPUs")]
        gpus: Option<usize>,
        /// Set the new job's log path
        #[arg(short, long, help = "Set the new job's log path")]
        log: Option<String>,
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
            help = "Show the latest N jobs; defaults to config or 20",
            conflicts_with = "all"
        )]
        limit: Option<usize>,
    },
    /// View job log
    Log {
        /// Job ID
        #[arg(help = "Job ID")]
        id: usize,
    },
    /// Manage persistent configuration
    Config {
        /// Configuration operation
        #[command(subcommand)]
        command: config::ConfigCommand,
    },
}

/// Parse command line and execute subcommands
pub(crate) fn run() {
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
            command,
            detach,
            follow,
            gpus,
            log,
        } => submit::run(command, detach, follow, gpus, log),
        Commands::Rerun {
            id,
            current_env,
            detach,
            follow,
            gpus,
            log,
        } => rerun::run(id, current_env, detach, follow, gpus, log),
        Commands::Cancel { id, force } => cancel::run(id, force),
        Commands::Delete { id, all, status } => delete::run(id, all, status),
        Commands::Status => status::run(),
        Commands::List { all, limit } => list::run(all, limit),
        Commands::Log { id } => log::run(id),
        Commands::Config { command } => config::run(command),
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Commands};

    #[test]
    fn configurable_arguments_preserve_explicit_values() {
        let cli = Cli::try_parse_from(["cgm", "start", "--interval", "5", "--scheduler", "fifo"])
            .unwrap();
        let Commands::Start {
            force,
            gpus,
            interval,
            scheduler,
            threshold,
        } = cli.command
        else {
            panic!("expected start command");
        };
        assert_eq!(force, false);
        assert!(gpus.is_none());
        assert_eq!(interval, Some(5));
        assert_eq!(scheduler, Some(crate::types::SchedulerKind::Fifo));
        assert!(threshold.is_none());
    }

    #[test]
    fn detach_and_follow_conflict() {
        assert!(
            Cli::try_parse_from(["cgm", "submit", "--detach", "--follow", "--", "true"]).is_err()
        );
        assert!(Cli::try_parse_from(["cgm", "rerun", "1", "--detach", "--follow"]).is_err());
    }
}
