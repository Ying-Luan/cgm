//! View job log subcommand
//!
//! Query job log path via daemon, then open with system less.

use std::{path::Path, process::exit};

use crate::{client::get_log_path, macros::yellow, os::open_log_with_less};

use super::utils::{exit_with_error, require_daemon};

/// View job log
///
/// # Arguments
///
/// * `id` - Job ID
pub(super) fn run(id: usize) {
    require_daemon();

    let log_path = match get_log_path(id) {
        Ok(p) => p,
        Err(e) => exit_with_error(&format!("Failed to get log path: {}", e)),
    };

    if !Path::new(&log_path).exists() {
        eprintln!("{}", yellow!("Log file not found: {}", log_path));
        eprintln!(
            "{}",
            yellow!("(Job may not have started writing output yet)")
        );
        exit(1);
    }

    // Open log file with system less and scroll to end
    if let Err(e) = open_log_with_less(&log_path, 0, false) {
        exit_with_error(&format!("Failed to open log viewer: {}", e));
    }
}
