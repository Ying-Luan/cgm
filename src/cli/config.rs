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
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        system: bool,
    },
    /// Set one value in the current configuration file.
    Set {
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        system: bool,
        /// Configuration key.
        key: String,
        /// Configuration value.
        value: String,
    },
    /// Remove one value from the configuration file.
    Unset {
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        system: bool,
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
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        system: bool,
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
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        system: bool,
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
        system: bool,
    },
    /// Print the current configuration file path.
    Path {
        /// Target the system-wide configuration.
        #[arg(
            long,
            default_value = "false",
            help = "Target the system-wide configuration"
        )]
        system: bool,
    },
}

/// Resolve the configuration scope selected by the caller.
///
/// # Arguments
///
/// * `system` - Whether `--system` was requested.
///
/// # Returns
///
/// The selected scope, or an error for an invalid caller/flag combination.
fn resolve_scope(system: bool, uid: Uid) -> Result<ConfigScope, String> {
    match (system, uid.is_root()) {
        (true, true) => Ok(ConfigScope::System),
        (true, false) => Err("This command must be run as sudo.".to_string()),
        (false, false) => Ok(ConfigScope::User),
        (false, true) => Err("Root must specify --system.".to_string()),
    }
}

/// Execute a configuration management operation.
///
/// # Arguments
///
/// * `command` - The configuration command to execute.
pub(super) fn run(command: ConfigCommand) {
    let scope = |system: bool| resolve_scope(system, Uid::effective());
    let result = match command {
        ConfigCommand::Init { force, system } => scope(system)
            .and_then(|scope| config::init(force, scope))
            .map(|path| {
                println!("{}", green!("Created config: {}", path.display()));
            }),
        ConfigCommand::Set { system, key, value } => scope(system)
            .and_then(|scope| config::set(&key, &value, scope))
            .map(|path| {
                println!("{}", green!("Updated config: {}", path.display()));
            }),
        ConfigCommand::Unset { system, key } => scope(system)
            .and_then(|scope| config::unset(&key, scope))
            .map(|(path, removed)| {
                if removed {
                    println!("{}", green!("Updated config: {}", path.display()));
                } else {
                    println!("{}", yellow!("Config key is not set: {}", key));
                }
            }),
        ConfigCommand::Get {
            effective,
            system,
            key,
            source,
        } => scope(system)
            .and_then(|scope| config::get(&key, effective, scope, source))
            .map(|value| {
                if !value.is_empty() {
                    println!("{}", value);
                }
            }),
        ConfigCommand::Show {
            effective,
            system,
            source,
        } => scope(system)
            .and_then(|scope| config::show(effective, scope, source))
            .map(|table| {
                if let Some(table) = table {
                    println!("{table}");
                } else {
                    println!("{}", yellow!("No configuration values are set."));
                }
            }),
        ConfigCommand::Validate { system } => {
            scope(system)
                .and_then(config::validate_current)
                .map(|path| {
                    println!("{}", green!("Config is valid: {}", path.display()));
                })
        }
        ConfigCommand::Path { system } => scope(system).and_then(config::config_path).map(|path| {
            println!("{}", path.display());
        }),
    };

    if let Err(error) = result {
        exit_with_error(&error);
    }
}

#[cfg(test)]
mod tests {
    use nix::unistd::Uid;

    use super::resolve_scope;
    use crate::types::ConfigScope;

    #[test]
    fn scope_requires_root_and_explicit_system() {
        let root = Uid::from_raw(0);
        let user = Uid::from_raw(1000);
        assert_eq!(resolve_scope(true, root), Ok(ConfigScope::System));
        assert!(resolve_scope(true, user).is_err());
        assert_eq!(resolve_scope(false, user), Ok(ConfigScope::User));
        assert!(resolve_scope(false, root).is_err());
    }
}
