//! Cancel job subcommand
//!
//! Send cancel request to daemon.

use crate::{cli::utils::require_daemon, client::cancel_job, macros::green};

use super::utils::exit_with_error;

/// Cancel job
///
/// # Arguments
///
/// * `id` - Job ID
/// * `force` - Force cancel running job
pub(super) fn run(id: usize, force: bool) {
    require_daemon();

    match cancel_job(id, force) {
        Ok(()) => println!("{}", green!("Job canceled successfully.")),
        Err(e) => exit_with_error(&format!("Failed to cancel job: {}", e)),
    }
}
