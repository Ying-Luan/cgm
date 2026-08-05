//! Persistent configuration loading and layer merging.

use std::{fs, path::Path};

use crate::{
    constants::GLOBAL_CONFIG_PATH,
    types::{Config, ConfigScope, EffectiveConfig, Source},
};

use super::{path::user_config_path, validate::validate_config};

/// Load the persistent configuration visible to the current effective UID.
///
/// # Returns
///
/// The merged effective configuration or an error message.
pub(crate) fn load_effective() -> Result<EffectiveConfig, String> {
    load_effective_from(
        Path::new(GLOBAL_CONFIG_PATH),
        Some(user_config_path()?.as_path()),
    )
}

/// Load only system-wide configuration and built-in defaults.
///
/// # Returns
///
/// The merged effective configuration or an error message.
pub(crate) fn load_global_effective() -> Result<EffectiveConfig, String> {
    load_effective_from(Path::new(GLOBAL_CONFIG_PATH), None)
}

/// Load and merge configuration files from explicit paths.
///
/// # Arguments
///
/// * `global_path` - The path to the global configuration file.
/// * `user_path` - An optional path to the user configuration file.
pub(super) fn load_effective_from(
    global_path: &Path,
    user_path: Option<&Path>,
) -> Result<EffectiveConfig, String> {
    let mut effective = EffectiveConfig::default();
    if let Some(global) = read_config(global_path, ConfigScope::Global)? {
        global.apply_to(&mut effective, Source::Global);
    }
    if let Some(user_path) = user_path
        && let Some(user) = read_config(user_path, ConfigScope::User)?
    {
        user.apply_to(&mut effective, Source::User);
    }

    Ok(effective)
}

/// Read and validate one optional configuration file.
///
/// # Arguments
///
/// * `path` - The path to the configuration file.
/// * `scope` - The scope of the configuration (global or user), used for validation.
///
/// # Returns
///
/// The parsed and validated configuration or an error message.
pub(super) fn read_config(path: &Path, scope: ConfigScope) -> Result<Option<Config>, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Failed to read config {}: {}",
                path.display(),
                error
            ));
        }
    };
    parse_config(&content, path, scope).map(Some)
}

/// Parse and validate one configuration document.
///
/// # Arguments
///
/// * `content` - The content of the configuration file.
/// * `path` - The path to the configuration file, used for error reporting.
/// * `scope` - The scope of the configuration (global or user), used for validation.
///
/// # Returns
///
/// The parsed and validated configuration or an error message.
pub(super) fn parse_config(
    content: &str,
    path: &Path,
    scope: ConfigScope,
) -> Result<Config, String> {
    let config: Config = toml_edit::de::from_str(content)
        .map_err(|error| format!("Failed to parse config {}: {}", path.display(), error))?;
    validate_config(&config, scope)?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, process, time::SystemTime};

    use crate::types::EffectiveConfig;

    use super::load_effective_from;

    /// Create an isolated temporary directory without an additional dependency.
    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cgm-{name}-{}-{unique}", process::id()));
        fs::create_dir(&path).unwrap();

        path
    }

    #[test]
    fn uses_built_in_defaults_without_files() {
        let dir = temp_dir("defaults");
        let config = load_effective_from(&dir.join("global.toml"), None).unwrap();
        assert_eq!(config, EffectiveConfig::default());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn merges_global_and_user_fields() {
        let dir = temp_dir("merge");
        let global = dir.join("global.toml");
        let user = dir.join("user.toml");
        fs::write(
            &global,
            "[submit]\ndetach = true\ngpus = 2\n[list]\nlimit = 40\n",
        )
        .unwrap();
        fs::write(&user, "[submit]\ngpus = 1\n").unwrap();

        let config = load_effective_from(&global, Some(&user)).unwrap();
        assert!(config.submit.detach.value);
        assert_eq!(config.submit.gpus.value, 1);
        assert_eq!(config.list.limit.value, 40);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_start_settings_in_user_config() {
        let dir = temp_dir("user-start");
        let user = dir.join("user.toml");
        fs::write(&user, "[start]\ninterval = 5\n").unwrap();

        let error = load_effective_from(&dir.join("global.toml"), Some(&user)).unwrap_err();
        assert!(error.contains("only allowed in the global config"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_unknown_and_invalid_values() {
        let dir = temp_dir("invalid");
        let global = dir.join("global.toml");
        fs::write(&global, "[submit]\ndetatch = true\n").unwrap();
        assert!(
            load_effective_from(&global, None)
                .unwrap_err()
                .contains("unknown field")
        );

        fs::write(&global, "[start]\ninterval = 0\n").unwrap();
        assert!(
            load_effective_from(&global, None)
                .unwrap_err()
                .contains("must be greater than 0")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
