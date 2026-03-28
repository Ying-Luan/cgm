//! View job log subcommand
//!
//! Query job log path via daemon, then open with system less.

use std::{
    path::Path,
    process::{self, exit},
};

use crate::{
    client::get_log_path,
    daemon::is_daemon_running,
    macros::{red, yellow},
    os::open_log_with_less,
};

/// View job log
///
/// # Arguments
///
/// * `id` - Job ID
pub fn run(id: usize) {
    // Check if daemon is running
    if !is_daemon_running() {
        println!("{}", yellow!("Daemon is not running."));
        exit(1);
    }

    let log_path = match get_log_path(id) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", red!("Failed to get log path: {}", e));
            process::exit(1);
        }
    };

    if !Path::new(&log_path).exists() {
        eprintln!("{}", yellow!("Log file not found: {}", log_path));
        eprintln!(
            "{}",
            yellow!("(Job may not have started writing output yet)")
        );
        process::exit(1);
    }

    // Open log file with system less and scroll to end
    if let Err(e) = open_log_with_less(&log_path, 0, false) {
        eprintln!("{}", red!("Failed to open log viewer: {}", e));
    }
}
