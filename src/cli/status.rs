//! Status subcommand
//!
//! Call monitor module to display GPU status.

use std::process::exit;

use crate::{daemon::is_daemon_running, macros::yellow, monitor::show_status};

/// View GPU status
pub fn run() {
    // Check if daemon is running
    if !is_daemon_running() {
        println!("{}", yellow!("Daemon is not running."));
        exit(1);
    }

    show_status()
}
