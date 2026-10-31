//! Configuration value validation.

use crate::{
    types::{Config, EffectiveConfig},
    utils::parse_comma_separated_ids,
};

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
    if config.list.limit.value == 0 {
        return Err("list.limit must be greater than 0".to_string());
    }
    if config.start.interval.value == 0 {
        return Err("start.interval must be greater than 0".to_string());
    }
    if config.start.threshold.value > 100 {
        return Err("start.threshold must be between 0 and 100".to_string());
    }
    if config.submit.required_gpus.value == 0 {
        return Err("submit.required_gpus must be greater than 0".to_string());
    }

    Ok(())
}

/// Validate values in one configuration file.
///
/// # Arguments
///
/// * `config` - The configuration to validate.
///
/// # Returns
///
/// Whether the configuration is valid or not.
pub(super) fn validate_config(config: &Config) -> Result<(), String> {
    if config.list.as_ref().and_then(|list| list.limit) == Some(0) {
        return Err("list.limit must be greater than 0".to_string());
    }
    if let Some(start) = &config.start {
        if let Some(gpus) = &start.gpus {
            validate_gpu_selection(gpus, "start.gpus")?;
        }
        if start.interval == Some(0) {
            return Err("start.interval must be greater than 0".to_string());
        }
        if start.threshold.is_some_and(|threshold| threshold > 100) {
            return Err("start.threshold must be between 0 and 100".to_string());
        }
    }
    if let Some(submit) = &config.submit {
        if let Some(allow_gpus) = &submit.allow_gpus {
            validate_gpu_selection(allow_gpus, "submit.allow_gpus")?;
        }
        if submit.required_gpus == Some(0) {
            return Err("submit.required_gpus must be greater than 0".to_string());
        }
    }

    Ok(())
}

/// Validate a GPU selection configuration value.
///
/// # Arguments
///
/// * `value` - GPU selection, either `all` or a comma-separated list of IDs.
/// * `key` - Configuration key used in error messages.
///
/// # Returns
///
/// Whether the GPU selection is valid or not.
fn validate_gpu_selection(value: &str, key: &str) -> Result<(), String> {
    if value == "all" {
        return Ok(());
    }
    parse_comma_separated_ids(Some(value))
        .map(|_| ())
        .map_err(|error| format!("Invalid {}: {}", key, error))
}
