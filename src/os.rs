//! OS utilities module
//!
//! Provides current user info and permission checks.

use nix::unistd::{Uid, User};
use std::process::{self, Command};

use crate::macros::red;

/// Get current username
///
/// Returns "unknown" if unable to get username
///
/// # Returns
///
/// A string representing the current username
pub fn get_current_username() -> String {
    match User::from_uid(Uid::current()) {
        Ok(Some(user)) => user.name,
        _ => "unknown".to_string(),
    }
}

/// Check if running as root
///
/// If not root, print error message and exit
pub fn require_root() {
    if !Uid::effective().is_root() {
        eprintln!("{}", red!("This command must be run as sudo."));
        process::exit(1);
    }
}

/// Open log file with system less
///
/// # Arguments
///
/// * `log_path` - Path to the log file
/// * `delay_ms` - Optional delay in milliseconds before opening less (to allow file creation)
/// * `follow` - Whether to open less in follow mode (like tail -f)
///
/// # Returns
///
/// Result indicating success or error message on failure
pub fn open_log_with_less(log_path: &str, delay_ms: u64, follow: bool) -> Result<(), String> {
    if delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }

    let mut cmd = Command::new("less");
    cmd.arg("-R");

    if follow {
        cmd.arg("+F");
    } else {
        cmd.arg("+G");
    }

    let status = cmd
        .arg(log_path)
        .status()
        .map_err(|e| format!("Failed to launch less: {}", e))?;

    if !status.success() {
        return Err(format!("less exited with status: {}", status));
    }

    Ok(())
}
