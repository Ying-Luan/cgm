//! Configuration data types.
//!
//! Defines partial persisted values, fully merged runtime values, source tracking,
//! and the conversions between them.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::{GpuSelection, SchedulerKind};

/// Configuration values read from one file.
#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    /// Job list defaults.
    pub(crate) list: Option<ListConfig>,
    /// Rerun defaults.
    pub(crate) rerun: Option<RerunConfig>,
    /// Daemon startup defaults.
    pub(crate) start: Option<StartConfig>,
    /// Job submission defaults.
    pub(crate) submit: Option<SubmitConfig>,
}

/// Job list values read from one configuration file.
#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ListConfig {
    /// Maximum number of jobs to display.
    pub(crate) limit: Option<usize>,
}

/// Rerun values read from one configuration file.
#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct RerunConfig {
    /// Whether to return without following the new job log.
    pub(crate) detach: Option<bool>,
}

/// Daemon startup values read from one configuration file.
#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct StartConfig {
    /// GPU IDs managed by the daemon, or `all`.
    pub(crate) gpus: Option<String>,
    /// Scheduling interval in seconds.
    pub(crate) interval: Option<u32>,
    /// Scheduling strategy.
    pub(crate) scheduler: Option<SchedulerKind>,
    /// External GPU occupation threshold.
    pub(crate) threshold: Option<u32>,
}

/// Job submission values read from one configuration file.
#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct SubmitConfig {
    /// GPU IDs allowed for submitted jobs, or `all`.
    pub(crate) allow_gpus: Option<String>,
    /// Whether to return without following the submitted job log.
    pub(crate) detach: Option<bool>,
    /// Number of GPUs requested by a job.
    pub(crate) required_gpus: Option<usize>,
}

/// Configuration value source.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Source {
    /// Built-in default.
    BuiltIn,
    /// System-wide configuration file.
    System,
    /// Current user's configuration file.
    User,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuiltIn => write!(f, "built-in"),
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
        }
    }
}

impl From<ConfigScope> for Source {
    fn from(scope: ConfigScope) -> Self {
        match scope {
            ConfigScope::System => Self::System,
            ConfigScope::User => Self::User,
        }
    }
}

impl Config {
    /// Apply explicitly configured fields to an effective configuration.
    ///
    /// # Arguments
    ///
    /// * `effective` - The effective configuration to which the fields will be applied.
    /// * `source` - The source layer of this configuration.
    pub(crate) fn apply_to(
        &self,
        effective: &mut EffectiveConfig,
        source: Source,
    ) -> Result<(), String> {
        if let Some(list) = &self.list
            && let Some(limit) = list.limit
        {
            effective.list.limit = WithSource {
                value: limit,
                source,
            };
        }
        if let Some(rerun) = &self.rerun
            && let Some(detach) = rerun.detach
        {
            effective.rerun.detach = WithSource {
                value: detach,
                source,
            };
        }
        if let Some(start) = &self.start {
            if let Some(gpus) = &start.gpus {
                effective.start.gpus = WithSource {
                    value: GpuSelection::parse(gpus)?,
                    source,
                };
            }
            if let Some(interval) = start.interval {
                effective.start.interval = WithSource {
                    value: interval,
                    source,
                };
            }
            if let Some(scheduler) = start.scheduler {
                effective.start.scheduler = WithSource {
                    value: scheduler,
                    source,
                };
            }
            if let Some(threshold) = start.threshold {
                effective.start.threshold = WithSource {
                    value: threshold,
                    source,
                };
            }
        }
        if let Some(submit) = &self.submit {
            if let Some(allow_gpus) = &submit.allow_gpus {
                effective.submit.allow_gpus = WithSource {
                    value: GpuSelection::parse(allow_gpus)?,
                    source,
                };
            }
            if let Some(detach) = submit.detach {
                effective.submit.detach = WithSource {
                    value: detach,
                    source,
                };
            }
            if let Some(required_gpus) = submit.required_gpus {
                effective.submit.required_gpus = WithSource {
                    value: required_gpus,
                    source,
                };
            }
        }

        Ok(())
    }
}

impl From<&EffectiveConfig> for Config {
    fn from(effective: &EffectiveConfig) -> Self {
        Self {
            list: Some(ListConfig {
                limit: Some(effective.list.limit.value),
            }),
            rerun: Some(RerunConfig {
                detach: Some(effective.rerun.detach.value),
            }),
            start: Some(StartConfig {
                gpus: Some(effective.start.gpus.value.to_string()),
                interval: Some(effective.start.interval.value),
                scheduler: Some(effective.start.scheduler.value),
                threshold: Some(effective.start.threshold.value),
            }),
            submit: Some(SubmitConfig {
                allow_gpus: Some(effective.submit.allow_gpus.value.to_string()),
                detach: Some(effective.submit.detach.value),
                required_gpus: Some(effective.submit.required_gpus.value),
            }),
        }
    }
}

/// A configuration value with its source.
#[derive(Debug)]
pub(crate) struct WithSource<T> {
    /// The resolved value.
    pub(crate) value: T,
    /// Where the value came from.
    pub(crate) source: Source,
}

impl<T> WithSource<T> {
    /// Create a value tracked to the built-in default source.
    pub(crate) fn builtin(value: T) -> Self {
        Self {
            value,
            source: Source::BuiltIn,
        }
    }
}

impl<T: fmt::Display> WithSource<T> {
    /// Get the value and source.
    ///
    /// # Returns
    ///
    /// A tuple containing the value as a string and its source.
    pub(crate) fn value_and_source(&self) -> (String, Source) {
        (self.value.to_string(), self.source)
    }
}

impl<T: PartialEq> PartialEq for WithSource<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

/// Fully merged configuration used by CLI commands.
#[derive(Debug, PartialEq)]
pub(crate) struct EffectiveConfig {
    /// Job list defaults.
    pub(crate) list: EffectiveListConfig,
    /// Rerun defaults.
    pub(crate) rerun: EffectiveRerunConfig,
    /// Daemon startup defaults.
    pub(crate) start: EffectiveStartConfig,
    /// Job submission defaults.
    pub(crate) submit: EffectiveSubmitConfig,
}

/// Fully resolved job list defaults.
#[derive(Debug, PartialEq)]
pub(crate) struct EffectiveListConfig {
    /// Maximum number of jobs to display.
    pub(crate) limit: WithSource<usize>,
}

/// Fully resolved rerun defaults.
#[derive(Debug, PartialEq)]
pub(crate) struct EffectiveRerunConfig {
    /// Whether to return without following the new job log.
    pub(crate) detach: WithSource<bool>,
}

/// Fully resolved daemon startup defaults.
#[derive(Debug, PartialEq)]
pub(crate) struct EffectiveStartConfig {
    /// GPU IDs managed by the daemon, or `all`.
    pub(crate) gpus: WithSource<GpuSelection>,
    /// Scheduling interval in seconds.
    pub(crate) interval: WithSource<u32>,
    /// Scheduling strategy.
    pub(crate) scheduler: WithSource<SchedulerKind>,
    /// External GPU occupation threshold.
    pub(crate) threshold: WithSource<u32>,
}

/// Fully resolved job submission defaults.
#[derive(Debug, PartialEq)]
pub(crate) struct EffectiveSubmitConfig {
    /// GPU IDs allowed for submitted jobs, or `all`.
    pub(crate) allow_gpus: WithSource<GpuSelection>,
    /// Whether to return without following the submitted job log.
    pub(crate) detach: WithSource<bool>,
    /// Number of GPUs requested by a job.
    pub(crate) required_gpus: WithSource<usize>,
}

impl Default for EffectiveConfig {
    fn default() -> Self {
        Self {
            list: EffectiveListConfig {
                limit: WithSource::builtin(20),
            },
            rerun: EffectiveRerunConfig {
                detach: WithSource::builtin(false),
            },
            start: EffectiveStartConfig {
                gpus: WithSource::builtin(GpuSelection::All),
                interval: WithSource::builtin(10),
                scheduler: WithSource::builtin(SchedulerKind::Greedy),
                threshold: WithSource::builtin(10),
            },
            submit: EffectiveSubmitConfig {
                allow_gpus: WithSource::builtin(GpuSelection::All),
                detach: WithSource::builtin(false),
                required_gpus: WithSource::builtin(1),
            },
        }
    }
}

/// Configuration file scope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ConfigScope {
    /// System-wide configuration.
    System,
    /// Current user's configuration.
    User,
}

impl From<bool> for ConfigScope {
    fn from(system: bool) -> Self {
        if system { Self::System } else { Self::User }
    }
}
