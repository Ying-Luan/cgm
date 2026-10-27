//! List jobs subcommand
//!
//! Query jobs via daemon, then display the list through the monitor module.

use crate::{
    client::list_jobs,
    config::{load_effective, validate_effective},
    monitor::show_list,
    types::EffectiveListConfig,
};

use super::utils::{exit_with_error, require_daemon};

/// Query and display jobs, exiting with an error if the query fails.
///
/// # Arguments
///
/// * `all` - If true, show all jobs without limit
/// * `limit` - Optional maximum number of jobs to show
/// * `long_format` - If true, show the detailed table with full field names and commands
pub(super) fn run(all: bool, limit: Option<usize>, long_format: bool) {
    require_daemon();

    let config = merge_config(limit).unwrap_or_else(|error| exit_with_error(&error));

    let limit_param = if all { None } else { Some(config.limit.value) };
    match list_jobs(limit_param) {
        Ok((jobs, total)) => show_list(&jobs, total, long_format),
        Err(error) => exit_with_error(&error),
    }
}

/// Merge CLI parameters and persistent configuration into effective list configuration.
///
/// # Arguments
///
/// * `limit` - Optional maximum number of jobs to show
///
/// # Returns
///
/// The effective list configuration, or an error message.
fn merge_config(limit: Option<usize>) -> Result<EffectiveListConfig, String> {
    let mut config = load_effective()?;
    if let Some(limit) = limit {
        config.list.limit.value = limit;
    }
    validate_effective(&config)?;

    Ok(config.list)
}
