//! View job info subcommand
//!
//! Query complete job details via daemon, then display saved fields with optional environment expansion.

use crate::{client::get_job_info, monitor::show_job_info};

use super::utils::{exit_with_error, require_daemon};

/// Query and display a job's saved details.
///
/// # Arguments
///
/// * `id` - Job ID to inspect.
/// * `env` - Show all saved environment variables.
pub(super) fn run(id: usize, env: bool) {
    require_daemon();

    match get_job_info(id) {
        Ok(job) => show_job_info(&job, env),
        Err(error) => exit_with_error(&error),
    }
}
