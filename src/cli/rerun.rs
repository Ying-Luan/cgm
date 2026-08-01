//! Rerun job subcommand
//!
//! Create a new job from an existing job's saved configuration.

use std::process::exit;

use crate::{
    client::rerun_job,
    daemon::is_daemon_running,
    macros::{green, red, yellow},
    os::open_log_with_less,
};

/// Rerun an existing job as a new job
///
/// # Arguments
///
/// * `id` - Source job ID
/// * `current_env` - Use the current environment instead of the saved environment
/// * `detach` - Detach without opening the log viewer
/// * `gpus` - Optional GPU count override
/// * `log` - Optional log path override
pub(crate) fn run(
    id: usize,
    current_env: bool,
    detach: bool,
    gpus: Option<usize>,
    log: Option<String>,
) {
    if !is_daemon_running() {
        println!("{}", yellow!("Daemon is not running."));
        exit(1);
    }

    let envs = current_env.then(|| std::env::vars().collect());
    match rerun_job(id, envs, gpus, log) {
        Ok((id, log_path)) => {
            println!(
                "{}",
                green!(
                    "Job rerun submitted successfully. ID: {}, Log: {}",
                    id,
                    log_path
                )
            );
            if !detach && let Err(e) = open_log_with_less(&log_path, 200, true) {
                eprintln!("{}", red!("Failed to open log viewer: {}", e));
            }
        }
        Err(e) => eprintln!("{}", red!("Failed to rerun job: {}", e)),
    }
}
