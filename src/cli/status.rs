//! Status subcommand
//!
//! Call monitor module to display GPU status.

use crate::{monitor::show_status, runtime::socket};

use super::utils::exit_with_error;

/// View GPU status
///
/// # Arguments
///
/// * `quiet` - Exit 0 if active, 1 if inactive, 2 if uncertain
pub(super) fn run(quiet: bool) {
    if quiet {
        let code = match socket::bind() {
            Ok(_) => 1,
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => 0,
            Err(_) => 2,
        };
        std::process::exit(code);
    }
    show_status().unwrap_or_else(|error| exit_with_error(&error));
}
