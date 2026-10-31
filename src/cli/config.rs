//! Configuration management subcommand.
//!
//! Dispatches configuration inspection and mutation operations.

use clap::Subcommand;
use nix::unistd::Uid;

use crate::{
    config,
    macros::{green, yellow},
    types::ConfigScope,
};

use super::utils::exit_with_error;

/// Configuration management operations.
#[derive(Subcommand)]
pub(super) enum ConfigCommand {
    /// Generate all default values for the current configuration scope.
    Init {
        /// Overwrite an existing configuration file.
        #[arg(
            short,
            long,
            default_value = "false",
            help = "Overwrite an existing configuration file"
        )]
        force: bool,
    },
    /// Set one value in the current configuration file.
    Set {
        /// Configuration key.
        key: String,
        /// Configuration value.
        value: String,
    },
    /// Remove one value from the configuration file.
    Unset {
        /// Configuration key.
        key: String,
    },
    /// Get one configuration value.
    Get {
        /// Show the merged effective value.
        #[arg(
            short,
            long,
            default_value = "false",
            help = "Show the merged effective value"
        )]
        effective: bool,
        /// Configuration key.
        key: String,
        /// Show the source of the value.
        #[arg(
            short,
            long,
            default_value = "false",
            help = "Show the source of the value"
        )]
        source: bool,
    },
    /// Show all configuration values as a table.
    Show {
        /// Show the merged effective value.
        #[arg(
            short,
            long,
            default_value = "false",
            help = "Show the merged effective value"
        )]
        effective: bool,
        /// Show the source of each value.
        #[arg(
            short,
            long,
            default_value = "false",
            help = "Show the source of each value"
        )]
        source: bool,
    },
    /// Validate the current configuration file.
    Validate,
    /// Print the current configuration file path.
    Path,
}

/// Execute a configuration management operation.
///
/// # Arguments
///
/// * `system` - Whether `--system` was requested.
/// * `command` - The configuration command to execute.
pub(super) fn run(system: bool, command: ConfigCommand) {
    let scope = match (system, Uid::effective().is_root()) {
        (true, true) => Ok(ConfigScope::System),
        (true, false) => Err("This command must be run as sudo.".to_string()),
        (false, false) => Ok(ConfigScope::User),
        (false, true) => Err("Root must specify --system.".to_string()),
    };

    let result = scope.and_then(|scope| match command {
        ConfigCommand::Init { force } => config::init(force, scope).map(|path| {
            println!("{}", green!("Created config: {}", path.display()));
        }),
        ConfigCommand::Set { key, value } => config::set(&key, &value, scope).map(|path| {
            println!("{}", green!("Updated config: {}", path.display()));
        }),
        ConfigCommand::Unset { key } => config::unset(&key, scope).map(|(path, removed)| {
            if removed {
                println!("{}", green!("Updated config: {}", path.display()));
            } else {
                println!("{}", yellow!("Config key is not set: {}", key));
            }
        }),
        ConfigCommand::Get {
            effective,
            key,
            source,
        } => config::get(&key, effective, scope, source).map(|value| {
            if !value.is_empty() {
                println!("{}", value);
            }
        }),
        ConfigCommand::Show { effective, source } => {
            config::show(effective, scope, source).map(|table| {
                if let Some(table) = table {
                    println!("{table}");
                } else {
                    println!("{}", yellow!("No configuration values are set."));
                }
            })
        }
        ConfigCommand::Validate => config::validate_current(scope).map(|path| {
            println!("{}", green!("Config is valid: {}", path.display()));
        }),
        ConfigCommand::Path => config::config_path(scope).map(|path| {
            println!("{}", path.display());
        }),
    });

    if let Err(error) = result {
        exit_with_error(&error);
    }
}
