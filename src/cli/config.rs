//! Configuration management subcommand.
//!
//! Dispatches configuration inspection and mutation operations.

use clap::Subcommand;

use crate::{
    config,
    macros::{green, yellow},
    os::require_root,
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
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        global: bool,
    },
    /// Set one value in the current configuration file.
    Set {
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        global: bool,
        /// Configuration key.
        key: String,
        /// Configuration value.
        value: String,
    },
    /// Remove one value from the current configuration layer.
    Unset {
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        global: bool,
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
            conflicts_with = "global",
            help = "Show the merged effective value"
        )]
        effective: bool,
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            conflicts_with = "effective",
            help = "Target the system-wide configuration"
        )]
        global: bool,
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
            conflicts_with = "global",
            help = "Show the merged effective value"
        )]
        effective: bool,
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            conflicts_with = "effective",
            help = "Target the system-wide configuration"
        )]
        global: bool,
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
    Validate {
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        global: bool,
    },
    /// Print the current configuration file path.
    Path {
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        global: bool,
    },
}

/// Execute a configuration management operation.
///
/// # Arguments
///
/// * `command` - The configuration command to execute.
pub(super) fn run(command: ConfigCommand) {
    let result = match command {
        ConfigCommand::Init { force, global } => {
            if global {
                require_root();
            }
            config::init(force, ConfigScope::from(global)).map(|path| {
                println!("{}", green!("Created config: {}", path.display()));
            })
        }
        ConfigCommand::Set { global, key, value } => {
            if global {
                require_root();
            }
            config::set(&key, &value, ConfigScope::from(global)).map(|path| {
                println!("{}", green!("Updated config: {}", path.display()));
            })
        }
        ConfigCommand::Unset { global, key } => {
            if global {
                require_root();
            }
            config::unset(&key, ConfigScope::from(global)).map(|(path, removed)| {
                if removed {
                    println!("{}", green!("Updated config: {}", path.display()));
                } else {
                    println!("{}", yellow!("Config key is not set: {}", key));
                }
            })
        }
        ConfigCommand::Get {
            effective,
            global,
            key,
            source,
        } => config::get(&key, effective, ConfigScope::from(global), source).map(|value| {
            if !value.is_empty() {
                println!("{}", value);
            }
        }),
        ConfigCommand::Show {
            effective,
            global,
            source,
        } => config::show(effective, ConfigScope::from(global), source).map(|table| {
            println!("{table}",);
        }),
        ConfigCommand::Validate { global } => config::validate_current(ConfigScope::from(global))
            .map(|path| {
                println!("{}", green!("Config is valid: {}", path.display()));
            }),
        ConfigCommand::Path { global } => {
            config::config_path(ConfigScope::from(global)).map(|path| {
                println!("{}", path.display());
            })
        }
    };

    if let Err(error) = result {
        exit_with_error(&error);
    }
}
