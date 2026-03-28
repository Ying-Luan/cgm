//! Program entry point
//!
//! Parse command line arguments

#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

mod cli;
mod client;
mod constants;
mod daemon;
mod db;
mod hardware;
mod macros;
mod monitor;
mod os;
mod types;

use crate::cli::run;

/// Parse command line and execute subcommands
fn main() {
    run();
}
