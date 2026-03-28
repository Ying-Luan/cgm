//! List jobs subcommand
//!
//! Call monitor module to display job list.

use std::process::exit;

use crate::{daemon::is_daemon_running, macros::yellow, monitor::show_list};

/// List jobs
///
/// # Arguments
///
/// * `all` - If true, show all jobs without limit
/// * `limit` - Maximum number of jobs to show when `all` is false
pub fn run(all: bool, limit: usize) {
    // Check if daemon is running
    if !is_daemon_running() {
        println!("{}", yellow!("Daemon is not running."));
        exit(1);
    }

    let limit_param = if all { None } else { Some(limit) };
    show_list(limit_param);
}
