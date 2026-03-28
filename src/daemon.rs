//! Daemon module
//!
//! Contains process (start/stop), scheduler (job scheduling), server (socket communication).

mod process;
mod scheduler;
mod server;

pub(crate) use process::{start_daemon, stop_daemon};
pub(crate) use server::is_daemon_running;
