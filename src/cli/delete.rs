//! Delete job subcommand
//!
//! Delete records of completed/failed/cancelled jobs.

use std::process::exit;

use crate::{
    client::delete_job,
    daemon::is_daemon_running,
    macros::{green, red, yellow},
    os::require_root,
    types::{DeleteTarget, JobStatus},
};

/// Delete job
///
/// # Arguments
///
/// * `id` - Job ID (optional)
/// * `all` - Delete all terminated jobs
/// * `status` - Delete by status (comma-separated)
pub fn run(id: Option<usize>, all: bool, status: Option<String>) {
    // Only root user can delete jobs
    require_root();

    // Check if daemon is running
    if !is_daemon_running() {
        println!("{}", yellow!("Daemon is not running."));
        exit(1);
    }

    // Determine delete target based on arguments
    let target = match (id, all, status) {
        (Some(id), false, None) => DeleteTarget::Single(id),
        (None, true, None) => DeleteTarget::AllTerminated,
        (None, false, Some(s)) => {
            let statuses: Vec<JobStatus> = s
                .split(',')
                .filter_map(|s| match s.trim() {
                    "completed" => Some(JobStatus::Completed),
                    "failed" => Some(JobStatus::Failed),
                    "cancelled" => Some(JobStatus::Cancelled),
                    _ => None,
                })
                .collect();
            DeleteTarget::ByStatuses(statuses)
        }
        _ => unreachable!(),
    };

    match delete_job(target) {
        Ok(count) => println!("{}", green!("Deleted {} job(s) successfully.", count)),
        Err(e) => eprintln!("{}", red!("Failed to delete job(s): {}", e)),
    }
}
