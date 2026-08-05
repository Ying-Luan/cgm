//! Configuration value and scope validation.

use crate::types::{Config, ConfigScope, EffectiveConfig};

/// Validate a fully merged configuration.
///
/// # Arguments
///
/// * `config` - The effective configuration to validate.
///
/// # Returns
///
/// Whether the effective configuration is valid or not.
pub(crate) fn validate_effective(config: &EffectiveConfig) -> Result<(), String> {
    validate_gpus(&config.start.gpus.value)?;
    if config.start.interval.value == 0 {
        return Err("start.interval must be greater than 0".to_string());
    }
    if config.start.threshold.value > 100 {
        return Err("start.threshold must be between 0 and 100".to_string());
    }
    if config.submit.gpus.value == 0 {
        return Err("submit.gpus must be greater than 0".to_string());
    }
    if config.list.limit.value == 0 {
        return Err("list.limit must be greater than 0".to_string());
    }

    Ok(())
}

/// Validate values and fields permitted in one configuration scope.
///
/// # Arguments
///
/// * `config` - The configuration to validate.
/// * `scope` - The scope of the configuration (global or user).
///
/// # Returns
///
/// Whether the configuration is valid or not.
pub(super) fn validate_config(config: &Config, scope: ConfigScope) -> Result<(), String> {
    if scope == ConfigScope::User && config.start.is_some() {
        return Err("start settings are only allowed in the global config".to_string());
    }
    if let Some(start) = &config.start {
        if let Some(gpus) = &start.gpus {
            validate_gpus(gpus)?;
        }
        if start.interval == Some(0) {
            return Err("start.interval must be greater than 0".to_string());
        }
        if start.threshold.is_some_and(|threshold| threshold > 100) {
            return Err("start.threshold must be between 0 and 100".to_string());
        }
    }
    if config.submit.as_ref().and_then(|submit| submit.gpus) == Some(0) {
        return Err("submit.gpus must be greater than 0".to_string());
    }
    if config.list.as_ref().and_then(|list| list.limit) == Some(0) {
        return Err("list.limit must be greater than 0".to_string());
    }

    Ok(())
}

/// Validate the syntax of a daemon GPU selection.
///
/// # Arguments
///
/// * `gpus` - A string containing the GPU selection.
///
/// # Returns
///
/// * Whether the GPU selection is valid or not.
fn validate_gpus(gpus: &str) -> Result<(), String> {
    if gpus == "all"
        || (!gpus.is_empty()
            && gpus
                .split(',')
                .all(|value| value.trim().parse::<usize>().is_ok()))
    {
        Ok(())
    } else {
        Err("start.gpus must be 'all' or a comma-separated list of GPU IDs".to_string())
    }
}
