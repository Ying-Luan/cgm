//! Stop daemon subcommand
//!
//! Check root permission, running jobs, then stop daemon.

use std::process;

use crate::{
    client::check_stop,
    daemon::stop_daemon,
    macros::{green, yellow},
    os::require_root,
};

use super::utils::{exit_with_error, require_daemon};

/// Stop daemon
///
/// # Arguments
///
/// * `force` - Force stop, ignores running jobs
pub(super) fn run(force: bool) {
    // Only root user can stop daemon
    require_root();

    require_daemon();

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
            Err(e) => exit_with_error(&format!("Failed to check running jobs: {}", e)),
        }
    }

    match stop_daemon() {
        Ok(pid) => println!("{}", green!("Daemon (PID {}) stopped.", pid)),
        Err(e) => exit_with_error(&format!("Failed to stop daemon: {}", e)),
    }
}
