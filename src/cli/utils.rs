//! Shared helpers for CLI subcommands.

use std::process::exit;

use crate::{
    daemon::is_daemon_running,
    macros::{red, yellow},
};

/// Maximum job name length in Unicode characters.
pub(super) const MAX_JOB_NAME_LENGTH: usize = 32;

/// Normalize and validate a job name supplied on the command line.
///
/// # Arguments
///
/// * `value` - The job name to parse
///
/// # Returns
///
/// The normalized job name, or an error message.
pub(super) fn parse_job_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Job name cannot be empty".into());
    }
    if value.chars().any(char::is_control) {
        return Err("Job name cannot contain control characters".into());
    }
    if value.chars().count() > MAX_JOB_NAME_LENGTH {
        return Err(format!(
            "Job name cannot exceed {MAX_JOB_NAME_LENGTH} characters"
        ));
    }
    Ok(value.to_string())
}

/// Normalize a nonempty name search string.
///
/// # Arguments
///
/// * `value` - The name filter to parse
///
/// # Returns
///
/// The normalized name filter, or an error message.
pub(super) fn parse_name_filter(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Name filter cannot be empty".into());
    }
    Ok(value.to_string())
}

/// Print an error and terminate the process.
///
/// # Arguments
///
/// * `error` - Error message to print
pub(super) fn exit_with_error(error: &str) -> ! {
    eprintln!("{}", red!("{}", error));
    exit(1);
}

/// Terminate with a warning when the daemon is not running.
pub(super) fn require_daemon() {
    if !is_daemon_running() {
        eprintln!("{}", yellow!("Daemon is not running."));
        exit(1);
    }
}
