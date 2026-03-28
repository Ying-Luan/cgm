//! Daemon module
//!
//! Contains process (start/stop), scheduler (job scheduling), server (socket communication).

mod process;
mod scheduler;
mod server;

pub use process::{start_daemon, stop_daemon};
pub use server::is_daemon_running;
