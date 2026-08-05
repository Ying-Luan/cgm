//! Delete job subcommand
//!
//! Delete records of completed/failed/cancelled jobs.

use crate::{
    client::delete_job,
    macros::green,
    os::require_root,
    types::{DeleteTarget, JobStatus},
};

use super::utils::{exit_with_error, require_daemon};

/// Delete job
///
/// # Arguments
///
/// * `id` - Job ID (optional)
/// * `all` - Delete all terminated jobs
/// * `status` - Delete by status (comma-separated)
pub(super) fn run(id: Option<usize>, all: bool, status: Option<String>) {
    // Only root user can delete jobs
    require_root();

    require_daemon();

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
        Err(e) => exit_with_error(&format!("Failed to delete job(s): {}", e)),
    }
}
