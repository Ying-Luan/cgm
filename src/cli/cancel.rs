//! Cancel job subcommand
//!
//! Send cancel request to daemon.

use crate::{cli::utils::require_daemon, client::cancel_job, macros::green};

use super::utils::exit_with_error;

/// Cancel job
///
/// # Arguments
///
/// * `ids` - Job IDs
/// * `force` - Force cancel running job
pub(super) fn run(ids: Vec<usize>, force: bool) {
    require_daemon();

    match cancel_job(ids, force) {
        Ok(()) => println!("{}", green!("Jobs canceled successfully.")),
        Err(error) => exit_with_error(&format!("Failed to cancel jobs: {error}")),
    }
}
