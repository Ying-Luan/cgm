//! Configuration key management and file persistence.
//!
//! Handles `config init/set/unset/get/show/validate` operations,
//! TOML document editing with comment preservation, and atomic writes.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process,
    str::FromStr,
};

use comfy_table::{Table, presets::NOTHING};
use toml_edit::{DocumentMut, Item, table, value};

use crate::types::{Config, ConfigScope, EffectiveConfig, Source};

use super::{
    load::{load_effective, parse_config, read_config},
    path::config_path,
};

/// Supported configuration keys.
#[derive(Clone, Copy)]
enum ConfigKey {
    /// `list.limit`.
    ListLimit,
    /// `rerun.detach`.
    RerunDetach,
    /// `start.gpus`.
    StartGpus,
    /// `start.interval`.
    StartInterval,
    /// `start.scheduler`.
    StartScheduler,
    /// `start.threshold`.
    StartThreshold,
    /// `submit.detach`.
    SubmitDetach,
    /// `submit.gpus`.
    SubmitGpus,
}

impl ConfigKey {
    /// All known configuration keys in display order.
    const ALL: [ConfigKey; 8] = [
        ConfigKey::StartGpus,
        ConfigKey::StartInterval,
        ConfigKey::StartScheduler,
        ConfigKey::StartThreshold,
        ConfigKey::SubmitDetach,
        ConfigKey::SubmitGpus,
        ConfigKey::RerunDetach,
        ConfigKey::ListLimit,
    ];

    /// Get the section, field, and dotted key string of this key.
    ///
    /// # Returns
    ///
    /// (section, field, key_string)
    fn names(&self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::ListLimit => ("list", "limit", "list.limit"),
            Self::RerunDetach => ("rerun", "detach", "rerun.detach"),
            Self::StartGpus => ("start", "gpus", "start.gpus"),
            Self::StartInterval => ("start", "interval", "start.interval"),
            Self::StartScheduler => ("start", "scheduler", "start.scheduler"),
            Self::StartThreshold => ("start", "threshold", "start.threshold"),
            Self::SubmitDetach => ("submit", "detach", "submit.detach"),
            Self::SubmitGpus => ("submit", "gpus", "submit.gpus"),
        }
    }

    /// Set this key in an editable TOML document.
    ///
    /// # Arguments
    ///
    /// * `document` - The TOML document to modify.
    /// * `raw_value` - The raw string value to set.
    ///
    /// # Returns
    ///
    /// Whether the operation was successful or not.
    fn set(&self, document: &mut DocumentMut, raw_value: &str) -> Result<(), String> {
        let (section, field, _) = self.names();
        if document.get(section).is_none() {
            document[section] = table();
        } else if !document[section].is_table_like() {
            return Err(format!("{} must be a table", section));
        }
        let decor = document
            .get(section)
            .and_then(Item::as_table_like)
            .and_then(|table| table.get(field))
            .and_then(Item::as_value)
            .map(|value| value.decor().clone());
        let mut item = match self {
            Self::ListLimit | Self::SubmitGpus => {
                let parsed = raw_value
                    .parse::<usize>()
                    .map_err(|_| format!("{}.{} must be a positive integer", section, field))?;
                let parsed = i64::try_from(parsed)
                    .map_err(|_| format!("{}.{} is too large", section, field))?;
                value(parsed)
            }
            Self::RerunDetach | Self::SubmitDetach => {
                let parsed = raw_value
                    .parse::<bool>()
                    .map_err(|_| format!("{}.{} must be true or false", section, field))?;
                value(parsed)
            }
            Self::StartGpus => value(raw_value),
            Self::StartInterval | Self::StartThreshold => {
                let parsed = raw_value
                    .parse::<u32>()
                    .map_err(|_| format!("{}.{} must be a non-negative integer", section, field))?;
                value(i64::from(parsed))
            }
            Self::StartScheduler => match raw_value {
                "fifo" | "greedy" => value(raw_value),
                _ => return Err("start.scheduler must be fifo or greedy".to_string()),
            },
        };
        if let Some(decor) = decor
            && let Some(value) = item.as_value_mut()
        {
            value.decor_mut().clone_from(&decor);
        }
        document[section][field] = item;

        Ok(())
    }

    /// Get this key's value and source from an effective configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The effective configuration from which to retrieve the value and source.
    ///
    /// # Returns
    ///
    /// The value and source of the configuration key.
    fn get(&self, config: &EffectiveConfig) -> (String, Source) {
        match self {
            Self::ListLimit => config.list.limit.value_and_source(),
            Self::RerunDetach => config.rerun.detach.value_and_source(),
            Self::StartGpus => config.start.gpus.value_and_source(),
            Self::StartInterval => config.start.interval.value_and_source(),
            Self::StartScheduler => config.start.scheduler.value_and_source(),
            Self::StartThreshold => config.start.threshold.value_and_source(),
            Self::SubmitDetach => config.submit.detach.value_and_source(),
            Self::SubmitGpus => config.submit.gpus.value_and_source(),
        }
    }

    /// Get this key's value from a raw configuration layer.
    ///
    /// # Arguments
    ///
    /// * `config` - The raw configuration from which to retrieve the value.
    ///
    /// # Returns
    ///
    /// The value of the configuration key, or `None` if not set.
    fn raw_value(&self, config: &Config) -> Option<String> {
        match self {
            Self::ListLimit => config.list.as_ref()?.limit.map(|v| v.to_string()),
            Self::RerunDetach => config.rerun.as_ref()?.detach.map(|v| v.to_string()),
            Self::StartGpus => config.start.as_ref()?.gpus.clone(),
            Self::StartInterval => config.start.as_ref()?.interval.map(|v| v.to_string()),
            Self::StartScheduler => config.start.as_ref()?.scheduler.map(|v| v.to_string()),
            Self::StartThreshold => config.start.as_ref()?.threshold.map(|v| v.to_string()),
            Self::SubmitDetach => config.submit.as_ref()?.detach.map(|v| v.to_string()),
            Self::SubmitGpus => config.submit.as_ref()?.gpus.map(|v| v.to_string()),
        }
    }

    /// Determine whether this key is allowed in a user configuration.
    ///
    /// # Returns
    ///
    /// Whether the key is allowed in a user configuration.
    fn user_allowed(&self) -> bool {
        !matches!(
            self,
            Self::StartGpus | Self::StartInterval | Self::StartScheduler | Self::StartThreshold
        )
    }
}

impl FromStr for ConfigKey {
    type Err = String;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        match key {
            "list.limit" => Ok(Self::ListLimit),
            "rerun.detach" => Ok(Self::RerunDetach),
            "start.gpus" => Ok(Self::StartGpus),
            "start.interval" => Ok(Self::StartInterval),
            "start.scheduler" => Ok(Self::StartScheduler),
            "start.threshold" => Ok(Self::StartThreshold),
            "submit.detach" => Ok(Self::SubmitDetach),
            "submit.gpus" => Ok(Self::SubmitGpus),
            _ => Err(format!("Unknown config key: {}", key)),
        }
    }
}

/// Generate the complete default configuration for the current scope.
///
/// # Arguments
///
/// * `force` - Whether to overwrite an existing configuration.
/// * `scope` - The configuration scope to target.
///
/// # Returns
///
/// The path to the generated configuration file or an error message.
pub(crate) fn init(force: bool, scope: ConfigScope) -> Result<PathBuf, String> {
    let path = config_path(scope)?;
    if path.exists() && !force {
        return Err(format!(
            "Config already exists: {}. Use --force to overwrite it",
            path.display()
        ));
    }

    let content = toml_edit::ser::to_string_pretty(&Config::from(&EffectiveConfig::default()))
        .map_err(|error| format!("Failed to generate default config: {}", error))?;
    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse generated default config: {}", error))?;
    if scope == ConfigScope::User {
        document.remove("start");
    }
    write_config(&document.to_string(), &path, scope)?;

    Ok(path)
}

/// Set one value in the current configuration file.
///
/// # Arguments
///
/// * `key` - The configuration key to set.
/// * `raw_value` - The raw string value to set.
/// * `scope` - The configuration scope to target.
///
/// # Returns
///
/// The path to the configuration file or an error message.
pub(crate) fn set(key: &str, raw_value: &str, scope: ConfigScope) -> Result<PathBuf, String> {
    let key = ConfigKey::from_str(key)?;
    let path = config_path(scope)?;
    require_key_scope(&key, scope)?;
    set_at(&key, &path, raw_value, scope)?;

    Ok(path)
}

/// Remove one value from the current configuration file.
///
/// # Arguments
///
/// * `key` - The configuration key to remove.
/// * `scope` - The configuration scope to target.
///
/// # Returns
///
/// The path to the configuration file and whether the key was removed or not, or an error message.
pub(crate) fn unset(key: &str, scope: ConfigScope) -> Result<(PathBuf, bool), String> {
    let key = ConfigKey::from_str(key)?;
    let path = config_path(scope)?;
    require_key_scope(&key, scope)?;
    let removed = unset_at(&key, &path, scope)?;

    Ok((path, removed))
}

/// Get one configuration value.
///
/// # Arguments
///
/// * `key` - The configuration key to retrieve.
/// * `effective` - Whether to show the merged effective value.
/// * `scope` - The configuration scope to target.
/// * `source` - Whether to append the value source.
///
/// # Returns
///
/// The value or an error message.
pub(crate) fn get(
    key: &str,
    effective: bool,
    scope: ConfigScope,
    source: bool,
) -> Result<String, String> {
    let key = ConfigKey::from_str(key)?;

    if !effective {
        let raw = read_config(&config_path(scope)?, scope)?;
        if let Some(raw) = raw
            && let Some(value) = key.raw_value(&raw)
        {
            if source {
                return Ok(format!("{} # {}", value, Source::from(scope)));
            }
            return Ok(value);
        }
        return Ok(String::new());
    }

    let (mut value, src) = key.get(&load_effective()?);
    if source {
        value.push_str(&format!(" # {}", src));
    }

    Ok(value)
}

/// Show all configuration values as a table.
///
/// # Arguments
///
/// * `effective` - Whether to show the merged effective value.
/// * `scope` - The configuration scope to target.
/// * `source` - Whether to include a source column.
///
/// # Returns
///
/// The table or an error message.
pub(crate) fn show(effective: bool, scope: ConfigScope, source: bool) -> Result<Table, String> {
    let config = load_effective()?;
    let raw = if !effective {
        read_config(&config_path(scope)?, scope)?
    } else {
        None
    };
    let all_keys = ConfigKey::ALL.iter().copied();

    let mut table = Table::new();
    table.load_preset(NOTHING);

    if source {
        table.set_header(vec!["Key", "Value", "Source"]);
    } else {
        table.set_header(vec!["Key", "Value"]);
    }

    for key in all_keys {
        let (value, src) = if !effective {
            match key.raw_value(raw.as_ref().unwrap()) {
                Some(v) => (v, None),
                None => continue,
            }
        } else {
            let (v, s) = key.get(&config);
            (v, Some(s))
        };

        if source {
            let src = src.unwrap_or_else(|| Source::from(scope));
            table.add_row(vec![key.names().2, &value, &src.to_string()]);
        } else {
            table.add_row(vec![key.names().2, &value]);
        }
    }

    Ok(table)
}

/// Validate the current scope's configuration file.
///
/// # Arguments
///
/// * `scope` - The configuration scope to target.
///
/// # Returns
///
/// The path to the validated configuration file or an error message.
pub(crate) fn validate_current(scope: ConfigScope) -> Result<PathBuf, String> {
    let path = config_path(scope)?;
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read config {}: {}", path.display(), error))?;
    parse_config(&content, &path, scope)?;

    Ok(path)
}

/// Set one value at an explicit path.
///
/// # Arguments
///
/// * `key` - The configuration key to set.
/// * `path` - The path to the configuration file.
/// * `raw_value` - The raw string value to set.
/// * `scope` - The scope of the configuration (global or user), used for validation.
///
/// # Returns
///
/// Whether the operation was successful or not.
fn set_at(key: &ConfigKey, path: &Path, raw_value: &str, scope: ConfigScope) -> Result<(), String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(format!(
                "Failed to read config {}: {}",
                path.display(),
                error
            ));
        }
    };
    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse config {}: {}", path.display(), error))?;
    key.set(&mut document, raw_value)?;

    let content = document.to_string();
    parse_config(&content, path, scope)?;
    write_config(&content, path, scope)?;

    Ok(())
}

/// Remove one value at an explicit path.
///
/// # Arguments
///
/// * `key` - The configuration key to remove.
/// * `path` - The path to the configuration file.
/// * `scope` - The scope of the configuration (global or user), used for validation.
///
/// # Returns
///
/// Whether the operation was successful or not.
fn unset_at(key: &ConfigKey, path: &Path, scope: ConfigScope) -> Result<bool, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "Failed to read config {}: {}",
                path.display(),
                error
            ));
        }
    };
    let mut document = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse config {}: {}", path.display(), error))?;
    let (section, field, _) = key.names();
    if document
        .get(section)
        .is_some_and(|item| !item.is_table_like())
    {
        return Err(format!("{} must be a table", section));
    }
    let removed = document
        .get_mut(section)
        .and_then(Item::as_table_like_mut)
        .and_then(|table| table.remove(field))
        .is_some();
    let remove_section = document
        .get(section)
        .and_then(Item::as_table_like)
        .is_some_and(|table| table.is_empty());
    if remove_section {
        document.remove(section);
    }

    if removed {
        let content = document.to_string();
        parse_config(&content, path, scope)?;
        write_config(&content, path, scope)?;
    }

    Ok(removed)
}

/// Reject keys that cannot be written in the selected scope.
///
/// # Arguments
///
/// * `key` - The configuration key to check.
/// * `scope` - The scope of the configuration (global or user).
///
/// # Returns
///
/// Whether the key is allowed in the selected scope or not.
fn require_key_scope(key: &ConfigKey, scope: ConfigScope) -> Result<(), String> {
    if scope == ConfigScope::User && !key.user_allowed() {
        Err("start settings are only allowed in the global config".to_string())
    } else {
        Ok(())
    }
}

/// Atomically replace a configuration file with the requested permissions.
///
/// # Arguments
///
/// * `content` - The content to write to the config file.
/// * `path` - The path to the configuration file.
/// * `scope` - The scope of the configuration (global or user), used for setting permissions.
///
/// # Returns
///
/// Whether the write was successful or not
fn write_config(content: &str, path: &Path, scope: ConfigScope) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Config path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "Failed to create config directory {}: {}",
            parent.display(),
            error
        )
    })?;
    let (directory_mode, file_mode) = match scope {
        ConfigScope::Global => (0o755, 0o644),
        ConfigScope::User => (0o700, 0o600),
    };
    fs::set_permissions(parent, fs::Permissions::from_mode(directory_mode)).map_err(|error| {
        format!(
            "Failed to set config directory permissions {}: {}",
            parent.display(),
            error
        )
    })?;

    let temporary_path = parent.join(format!(".config.toml.{}.tmp", process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .mode(file_mode)
            .write(true)
            .open(&temporary_path)
            .map_err(|error| {
                format!(
                    "Failed to create temporary config {}: {}",
                    temporary_path.display(),
                    error
                )
            })?;
        file.write_all(content.as_bytes()).map_err(|error| {
            format!(
                "Failed to write temporary config {}: {}",
                temporary_path.display(),
                error
            )
        })?;
        file.sync_all().map_err(|error| {
            format!(
                "Failed to sync temporary config {}: {}",
                temporary_path.display(),
                error
            )
        })?;
        fs::rename(&temporary_path, path)
            .map_err(|error| format!("Failed to replace config {}: {}", path.display(), error))?;
        fs::set_permissions(path, fs::Permissions::from_mode(file_mode)).map_err(|error| {
            format!(
                "Failed to set config permissions {}: {}",
                path.display(),
                error
            )
        })
    })();
    if result.is_err() {
        fs::remove_file(temporary_path).ok();
    }

    result
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, process, str::FromStr, time::SystemTime};

    use super::{ConfigKey, ConfigScope, set_at, unset_at};
    use crate::config::load::load_effective_from;

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
    fn set_preserves_existing_comments() {
        let dir = temp_dir("comments");
        let path = dir.join("config.toml");
        fs::write(
            &path,
            "# Submission defaults\n[submit]\n# GPU count\ngpus   = 1 # per job\n",
        )
        .unwrap();

        set_at(
            &ConfigKey::from_str("submit.gpus").unwrap(),
            &path,
            "2",
            ConfigScope::Global,
        )
        .unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Submission defaults"));
        assert!(content.contains("# GPU count"));
        assert!(content.contains("gpus   = 2 # per job"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn set_and_unset_can_repair_invalid_target_values() {
        let dir = temp_dir("repair");
        let path = dir.join("config.toml");
        fs::write(&path, "[submit]\ngpus = \"invalid\"\n").unwrap();

        let key = ConfigKey::from_str("submit.gpus").unwrap();
        set_at(&key, &path, "2", ConfigScope::User).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("gpus = 2"));

        fs::write(&path, "[submit]\ngpus = \"invalid\"\n").unwrap();
        assert!(unset_at(&key, &path, ConfigScope::User).unwrap());
        assert!(!fs::read_to_string(&path).unwrap().contains("submit"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_non_table_sections_without_panicking() {
        let dir = temp_dir("non-table");
        let path = dir.join("config.toml");
        let key = ConfigKey::from_str("submit.gpus").unwrap();
        fs::write(&path, "submit = 1\n").unwrap();

        assert!(
            set_at(&key, &path, "2", ConfigScope::User)
                .unwrap_err()
                .contains("submit must be a table")
        );
        assert!(
            unset_at(&key, &path, ConfigScope::User)
                .unwrap_err()
                .contains("submit must be a table")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn set_creates_a_minimal_config() {
        let dir = temp_dir("bootstrap");
        let path = dir.join("config.toml");
        set_at(
            &ConfigKey::from_str("submit.detach").unwrap(),
            &path,
            "true",
            ConfigScope::User,
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[submit]\ndetach = true\n"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unset_falls_back_to_global_value() {
        let dir = temp_dir("unset");
        let global = dir.join("global.toml");
        let user = dir.join("user.toml");
        fs::write(&global, "[submit]\ndetach = true\n").unwrap();
        fs::write(&user, "submit = { detach = false }\n").unwrap();

        let removed = unset_at(
            &ConfigKey::from_str("submit.detach").unwrap(),
            &user,
            ConfigScope::User,
        )
        .unwrap();
        assert!(removed);
        let config = load_effective_from(&global, Some(&user)).unwrap();
        assert!(config.submit.detach.value);
        assert!(!fs::read_to_string(&user).unwrap().contains("submit"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unset_is_idempotent_without_a_config_file() {
        let dir = temp_dir("unset-missing");
        let removed = unset_at(
            &ConfigKey::from_str("submit.detach").unwrap(),
            &dir.join("config.toml"),
            ConfigScope::User,
        )
        .unwrap();
        assert!(!removed);
        fs::remove_dir_all(dir).unwrap();
    }
}
