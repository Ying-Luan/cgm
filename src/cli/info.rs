//! View job info subcommand
//!
//! Query complete job details via daemon, then display all saved fields and dependencies.

use crate::{client::get_job_info, monitor::show_job_info};

use super::utils::{exit_with_error, require_daemon};

/// Query and display a job's saved details.
///
/// # Arguments
///
/// * `id` - Job ID to inspect.
pub(super) fn run(id: usize) {
    require_daemon();

    match get_job_info(id) {
        Ok(job) => show_job_info(&job),
        Err(error) => exit_with_error(&error),
    }
}
