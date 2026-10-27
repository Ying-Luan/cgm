//! Rerun job subcommand
//!
//! Create a new job from an existing job's saved configuration.

use std::str::FromStr;

use crate::{
    client::rerun_job,
    config::{load_effective, validate_effective},
    macros::green,
    os::open_log_with_less,
    types::{EffectiveRerunConfig, GpuSelection, RerunContext},
    utils::parse_comma_separated_ids,
};

use super::utils::{exit_with_error, require_daemon};

/// Fields to replace with values from the current terminal.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct CurrentFields {
    /// Whether to use the current working directory.
    dir: bool,
    /// Whether to use the current environment variables.
    env: bool,
}

impl FromStr for CurrentFields {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut fields = Self {
            dir: false,
            env: false,
        };
        for field in value.split(',') {
            let (selected, name) = match field {
                "dir" | "d" => (&mut fields.dir, "dir"),
                "env" | "e" => (&mut fields.env, "env"),
                _ => {
                    return Err(format!(
                        "Invalid current field '{field}'; expected env (e) or dir (d)"
                    ));
                }
            };
            if *selected {
                return Err(format!("Duplicate current field '{name}'"));
            }
            *selected = true;
        }
        Ok(fields)
    }
}

/// Rerun an existing job as a new job
///
/// # Arguments
///
/// * `id` - Source job ID
/// * `allow_gpus` - Optional comma-separated GPU IDs this job may use
/// * `current` - Fields to replace with values from the current terminal
/// * `depends_on` - Optional comma-separated prerequisite job IDs
/// * `detach` - Detach flag, returns immediately after submission
/// * `follow` - Follow flag, opens less to follow the new job log
/// * `log` - Optional log path override
/// * `require_gpus` - Optional GPU count override
pub(super) fn run(
    id: usize,
    allow_gpus: Option<String>,
    current: Option<CurrentFields>,
    depends_on: Option<String>,
    detach: bool,
    follow: bool,
    log: Option<String>,
    require_gpus: Option<usize>,
) {
    require_daemon();

    if require_gpus == Some(0) {
        exit_with_error("required-gpus must be greater than 0");
    }
    let config = merge_config(detach, follow).unwrap_or_else(|error| exit_with_error(&error));
    let allowed_gpus = allow_gpus
        .map(|value| GpuSelection::parse(&value))
        .transpose()
        .unwrap_or_else(|error| exit_with_error(&error));
    let dependencies = parse_comma_separated_ids(depends_on.as_deref())
        .unwrap_or_else(|error| exit_with_error(&error));

    let context = RerunContext {
        cwd: if current.as_ref().is_some_and(|fields| fields.dir) {
            Some(
                std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
        } else {
            None
        },
        envs: current
            .as_ref()
            .is_some_and(|fields| fields.env)
            .then(|| std::env::vars().collect()),
    };
    match rerun_job(id, context, allowed_gpus, dependencies, require_gpus, log) {
        Ok((id, log_path)) => {
            println!(
                "{}",
                green!(
                    "Job rerun submitted successfully. ID: {}, Log: {}",
                    id,
                    log_path
                )
            );
            if !config.detach.value
                && let Err(e) = open_log_with_less(&log_path, 200, true)
            {
                exit_with_error(&format!("Failed to open log viewer: {}", e));
            }
        }
        Err(e) => exit_with_error(&format!("Failed to rerun job: {}", e)),
    }
}

/// Merge CLI parameters and persistent configuration into effective rerun configuration.
///
/// # Arguments
///
/// * `detach` - Detach flag from CLI
/// * `follow` - Follow flag from CLI
///
/// # Returns
///
/// The effective rerun configuration or an error message.
fn merge_config(detach: bool, follow: bool) -> Result<EffectiveRerunConfig, String> {
    let mut config = load_effective()?;
    if detach {
        config.rerun.detach.value = true;
    } else if follow {
        config.rerun.detach.value = false;
    };
    validate_effective(&config)?;

    Ok(config.rerun)
}
