//! Cancel job subcommand
//!
//! Send cancel request to daemon.

use std::process::exit;

use crate::{
    client::cancel_job,
    daemon::is_daemon_running,
    macros::{green, red, yellow},
};

/// Cancel job
///
/// # Arguments
///
/// * `id` - Job ID
/// * `force` - Force cancel running job
pub fn run(id: usize, force: bool) {
    // Check if daemon is running
    if !is_daemon_running() {
        println!("{}", yellow!("Daemon is not running."));
        exit(1);
    }

    match cancel_job(id, force) {
        Ok(()) => println!("{}", green!("Job canceled successfully.")),
        Err(e) => eprintln!("{}", red!("Failed to cancel job: {}", e)),
    }
}
