//! Submit job subcommand
//!
//! Build JobRequest and send to daemon via socket.

use crate::{
    client::submit_job,
    config::{load_effective, validate_effective},
    macros::green,
    os::{get_current_username, open_log_with_less},
    types::{EffectiveSubmitConfig, JobRequest},
};

use super::utils::{exit_with_error, require_daemon};

/// Submit job to daemon
///
/// # Arguments
///
/// * `command` - Command to execute and arguments
/// * `detach` - Detach flag, returns immediately after submission
/// * `follow` - Follow flag, opens less to follow the job log
/// * `gpus` - Optional number of GPUs to request
/// * `log` - Optional log file path
pub(super) fn run(
    command: Vec<String>,
    detach: bool,
    follow: bool,
    gpus: Option<usize>,
    log: Option<String>,
) {
    require_daemon();

    let config = merge_config(detach, follow, gpus).unwrap_or_else(|error| exit_with_error(&error));

    let request = JobRequest {
        username: get_current_username(),
        command,
        gpus: config.gpus.value,
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
            if !config.detach.value
                && let Err(e) = open_log_with_less(&log_path, 200, true)
            {
                exit_with_error(&format!("Failed to open log viewer: {}", e));
            }
        }
        Err(e) => exit_with_error(&format!("Failed to submit job: {}", e)),
    }
}

/// Merge CLI parameters and persistent configuration into effective submit options.
///
/// # Arguments
///
/// * `detach` - Detach flag from CLI
/// * `follow` - Follow flag from CLI
/// * `gpus` - Optional number of GPUs from CLI
///
/// # Returns
///
/// The effective submit configuration or an error message.
fn merge_config(
    detach: bool,
    follow: bool,
    gpus: Option<usize>,
) -> Result<EffectiveSubmitConfig, String> {
    let mut config = load_effective()?;
    if detach {
        config.submit.detach.value = true;
    } else if follow {
        config.submit.detach.value = false;
    }
    if let Some(gpus) = gpus {
        config.submit.gpus.value = gpus;
    }
    validate_effective(&config)?;

    Ok(config.submit)
}
