//! List jobs subcommand
//!
//! Call monitor module to display job list.

use crate::{
    config::{load_effective, validate_effective},
    monitor::show_list,
    types::EffectiveListConfig,
};

use super::utils::{exit_with_error, require_daemon};

/// List jobs
///
/// # Arguments
///
/// * `all` - If true, show all jobs without limit
/// * `limit` - Optional maximum number of jobs to show
pub(super) fn run(all: bool, limit: Option<usize>) {
    require_daemon();

    let config = merge_config(limit).unwrap_or_else(|error| exit_with_error(&error));

    let limit_param = if all { None } else { Some(config.limit.value) };
    show_list(limit_param);
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
