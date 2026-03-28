//! Stop daemon subcommand
//!
//! Check root permission, running jobs, then stop daemon.

use std::process;

use crate::{
    client::check_stop,
    daemon::{is_daemon_running, stop_daemon},
    macros::{green, red, yellow},
    os::require_root,
};

/// Stop daemon
///
/// # Arguments
///
/// * `force` - Force stop, ignores running jobs
pub fn run(force: bool) {
    // Only root user can stop daemon
    require_root();

    // Check if daemon is running
    if !is_daemon_running() {
        println!("{}", yellow!("Daemon is not running."));
        return;
    }

    // If --force not specified, check for running jobs
    if !force {
        match check_stop() {
            Ok(count) if count > 0 => {
                eprintln!(
                    "{}",
                    yellow!(
                        "There are {} running job(s). Use --force to stop anyway.",
                        count
                    )
                );
                process::exit(1);
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("{}", red!("Failed to check running jobs: {}", e));
                process::exit(1);
            }
        }
    }

    match stop_daemon() {
        Ok(pid) => println!("{}", green!("Daemon (PID {}) stopped.", pid)),
        Err(e) => eprintln!("{}", red!("Failed to stop daemon: {}", e)),
    }
}
