//! Daemon module
//!
//! Coordinate startup, job execution, scheduling strategies, and socket requests.

mod control;
mod executor;
mod process;
mod scheduler;
mod server;

pub(crate) use process::start_daemon;
pub(crate) use server::is_daemon_running;
