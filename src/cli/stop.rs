//! Stop daemon subcommand
//!
//! Request an authorized daemon shutdown.

use crate::{client, macros::green};

use super::utils::exit_with_error;

/// Stop daemon
///
/// # Arguments
///
/// * `force` - Terminate running tasks and wait for cleanup before stopping.
pub(super) fn run(force: bool) {
    match client::stop(force) {
        Ok(()) => println!("{}", green!("Daemon stopped.")),
        Err(e) => exit_with_error(&format!("Failed to stop daemon: {}", e)),
    }
}
