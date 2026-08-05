//! Configuration scope and path resolution.

use std::{env, path::PathBuf};

use nix::unistd::{Uid, User};

use crate::{constants::GLOBAL_CONFIG_PATH, types::ConfigScope};

/// Get the configuration path for the given scope.
///
/// # Arguments
///
/// * `scope` - The configuration scope for which to get the path.
///
/// # Returns
///
/// The configuration path for the given scope or an error message.
pub(crate) fn config_path(scope: ConfigScope) -> Result<PathBuf, String> {
    match scope {
        ConfigScope::Global => Ok(PathBuf::from(GLOBAL_CONFIG_PATH)),
        ConfigScope::User => user_config_path(),
    }
}

/// Resolve the current user's XDG configuration path.
///
/// # Returns
///
/// The resolved path to the user's configuration file or an error message.
pub(super) fn user_config_path() -> Result<PathBuf, String> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME")
        && !config_home.is_empty()
    {
        let config_home = PathBuf::from(config_home);
        if !config_home.is_absolute() {
            return Err("XDG_CONFIG_HOME must be an absolute path".to_string());
        }
        return Ok(config_home.join("cgm/config.toml"));
    }

    let user = User::from_uid(Uid::effective())
        .map_err(|error| format!("Failed to resolve current user: {}", error))?
        .ok_or_else(|| "Failed to resolve current user's home directory".to_string())?;
    Ok(user.dir.join(".config/cgm/config.toml"))
}
