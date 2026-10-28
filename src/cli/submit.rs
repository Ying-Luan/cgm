//! Submit job subcommand
//!
//! Submit a job via daemon, display its saved details.

use crate::{
    client::submit_job,
    config::{load_effective, validate_effective},
    macros::green,
    monitor::show_job_info,
    os::{get_current_username, open_log_with_less},
    types::{EffectiveSubmitConfig, GpuSelection, JobRequest},
    utils::parse_comma_separated_ids,
};

use super::utils::{exit_with_error, require_daemon};

/// Submit job to daemon
///
/// # Arguments
///
/// * `allow_gpus` - Optional comma-separated GPU IDs this job may use
/// * `command` - Command to execute and arguments
/// * `depends_on` - Optional comma-separated prerequisite job IDs
/// * `detach` - Detach flag, returns immediately after submission
/// * `follow` - Follow flag, opens less to follow the job log
/// * `log` - Optional log file path
/// * `require_gpus` - Optional number of GPUs to request
pub(super) fn run(
    allow_gpus: Option<String>,
    command: Vec<String>,
    depends_on: Option<String>,
    detach: bool,
    follow: bool,
    log: Option<String>,
    require_gpus: Option<usize>,
) {
    require_daemon();

    let config = merge_config(allow_gpus, detach, follow, require_gpus)
        .unwrap_or_else(|error| exit_with_error(&error));
    let dependencies = parse_comma_separated_ids(depends_on.as_deref())
        .unwrap_or_else(|error| exit_with_error(&error))
        .unwrap_or_default();

    if config
        .allow_gpus
        .value
        .len()
        .is_some_and(|allowed_count| config.required_gpus.value > allowed_count)
    {
        exit_with_error("require-gpus cannot exceed the number of allowed GPUs");
    }

    let request = JobRequest {
        username: get_current_username(),
        allowed_gpus: config.allow_gpus.value,
        command,
        dependencies,
        required_gpus: config.required_gpus.value,
        envs: std::env::vars().collect(),
        log_path: log,
        cwd: std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    };

    match submit_job(request) {
        Ok(job) => {
            println!("{}", green!("Job submitted successfully."));
            show_job_info(&job, false);
            if !config.detach.value
                && let Err(e) = open_log_with_less(&job.log_path, 200, true)
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
/// * `allow_gpus` - Optional comma-separated GPU IDs from CLI
/// * `detach` - Detach flag from CLI
/// * `follow` - Follow flag from CLI
/// * `require_gpus` - Optional number of GPUs from CLI
///
/// # Returns
///
/// The effective submit configuration or an error message.
fn merge_config(
    allow_gpus: Option<String>,
    detach: bool,
    follow: bool,
    require_gpus: Option<usize>,
) -> Result<EffectiveSubmitConfig, String> {
    let mut config = load_effective()?;
    if let Some(allow_gpus) = allow_gpus {
        config.submit.allow_gpus.value = GpuSelection::parse(&allow_gpus)?;
    }
    if detach {
        config.submit.detach.value = true;
    } else if follow {
        config.submit.detach.value = false;
    }
    if let Some(require_gpus) = require_gpus {
        config.submit.required_gpus.value = require_gpus;
    }
    validate_effective(&config)?;

    Ok(config.submit)
}
