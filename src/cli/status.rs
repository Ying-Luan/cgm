//! Status subcommand
//!
//! Call monitor module to display GPU status.

use crate::monitor::show_status;

use super::utils::require_daemon;

/// View GPU status
pub(super) fn run() {
    require_daemon();

    show_status()
}
