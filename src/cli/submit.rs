//! Submit job subcommand
//!
//! Build JobRequest and send to daemon via socket.

use std::process::exit;

use crate::{
    client::submit_job,
    daemon::is_daemon_running,
    macros::{green, red, yellow},
    os::{get_current_username, open_log_with_less},
    types::JobRequest,
};

/// Submit job to daemon
///
/// # Arguments
///
/// * `detach` - Detach mode, opens less to follow log after submission
/// * `gpus` - Number of GPUs needed
/// * `log` - Log file path
/// * `command` - Command to execute and arguments
pub fn run(detach: bool, gpus: usize, log: Option<String>, command: Vec<String>) {
    // Check if daemon is running
    if !is_daemon_running() {
        println!("{}", yellow!("Daemon is not running."));
        exit(1);
    }

    let request = JobRequest {
        username: get_current_username(),
        command,
        gpus,
        envs: std::env::vars().collect(),
        log_path: log,
        cwd: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    };

    match submit_job(request) {
        Ok((id, log_path)) => {
            println!(
                "{}",
                green!("Job submitted successfully. ID: {}, Log: {}", id, log_path)
            );
            if !detach && let Err(e) = open_log_with_less(&log_path, 200, true) {
                eprintln!("{}", red!("Failed to open log viewer: {}", e));
            }
        }
        Err(e) => eprintln!("{}", red!("Failed to submit job: {}", e)),
    }
}
