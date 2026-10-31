//! Persistent configuration loading and layer merging.

use std::{fs, path::Path};

use nix::unistd::Uid;

use crate::types::{Config, ConfigScope, EffectiveConfig, Source};

use super::{path::config_path, validate::validate_config};

/// Load the configuration selected by the effective UID.
///
/// # Returns
///
/// The merged effective configuration or an error message.
pub(crate) fn load_effective() -> Result<EffectiveConfig, String> {
    load_scope_effective(ConfigScope::from(Uid::effective().is_root()))
}

/// Load one configuration scope over built-in defaults.
///
/// # Arguments
///
/// * `scope` - The configuration file to load.
///
/// # Returns
///
/// The merged effective configuration or an error message.
pub(super) fn load_scope_effective(scope: ConfigScope) -> Result<EffectiveConfig, String> {
    let mut effective = EffectiveConfig::default();
    if let Some(config) = read_config(&config_path(scope)?)? {
        config.apply_to(&mut effective, Source::from(scope))?;
    }

    Ok(effective)
}

/// Read and validate one optional configuration file.
///
/// # Arguments
///
/// * `path` - The path to the configuration file.
///
/// # Returns
///
/// The parsed and validated configuration or an error message.
pub(super) fn read_config(path: &Path) -> Result<Option<Config>, String> {
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
    parse_config(&content, path).map(Some)
}

/// Parse and validate one configuration document.
///
/// # Arguments
///
/// * `content` - The content of the configuration file.
/// * `path` - The path to the configuration file, used for error reporting.
///
/// # Returns
///
/// The parsed and validated configuration or an error message.
pub(super) fn parse_config(content: &str, path: &Path) -> Result<Config, String> {
    let config: Config = toml_edit::de::from_str(content)
        .map_err(|error| format!("Failed to parse config {}: {}", path.display(), error))?;
    validate_config(&config)?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, process, time::SystemTime};

    use super::read_config;

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
    fn rejects_unknown_and_invalid_values() {
        let dir = temp_dir("invalid");
        let path = dir.join("config.toml");
        fs::write(&path, "[submit]\ndetatch = true\n").unwrap();
        assert!(read_config(&path).err().unwrap().contains("unknown field"));

        fs::write(&path, "[start]\ninterval = 0\n").unwrap();
        assert!(
            read_config(&path)
                .err()
                .unwrap()
                .contains("must be greater than 0")
        );

        fs::write(&path, "[start]\ngpus = \"0,0\"\n").unwrap();
        assert!(
            read_config(&path)
                .err()
                .unwrap()
                .contains("Duplicate ID: 0")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
