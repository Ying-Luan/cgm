//! Delete job subcommand
//!
//! Request deletion of terminated jobs.

use crate::{
    client::delete_job,
    macros::green,
    types::{DeleteTarget, JobStatus},
};

use super::utils::{exit_with_error, require_daemon};

/// Delete job
///
/// # Arguments
///
/// * `ids` - Job IDs (optional)
/// * `all` - Delete all terminated jobs
/// * `status` - Terminal statuses to delete
pub(super) fn run(ids: Option<Vec<usize>>, all: bool, status: Option<Vec<JobStatus>>) {
    require_daemon();

    // Determine delete target based on arguments
    let target = match (ids, all, status) {
        (Some(ids), false, None) => DeleteTarget::Ids(ids),
        (None, true, None) => DeleteTarget::AllTerminated,
        (None, false, Some(statuses)) => DeleteTarget::ByStatuses(statuses),
        _ => unreachable!(),
    };

    match delete_job(target) {
        Ok(count) => println!("{}", green!("Deleted {} job(s) successfully.", count)),
        Err(e) => exit_with_error(&format!("Failed to delete job(s): {}", e)),
    }
}
