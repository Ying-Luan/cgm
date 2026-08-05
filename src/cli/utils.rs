//! Shared helpers for CLI subcommands.

use std::process::exit;

use crate::{
    daemon::is_daemon_running,
    macros::{red, yellow},
};

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
        println!("{}", yellow!("Daemon is not running."));
        exit(1);
    }
}
