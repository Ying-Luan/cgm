//! Configuration data types.
//!
//! Defines partial persisted values, fully merged runtime values, source tracking,
//! and the conversions between them.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::SchedulerKind;

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
    /// Whether to return without following the submitted job log.
    pub(crate) detach: Option<bool>,
    /// Number of GPUs requested by a job.
    pub(crate) gpus: Option<usize>,
}

/// Configuration value source.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Source {
    /// Built-in default.
    BuiltIn,
    /// System-wide configuration file.
    Global,
    /// Current user's configuration file.
    User,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuiltIn => write!(f, "built-in"),
            Self::Global => write!(f, "global"),
            Self::User => write!(f, "user"),
        }
    }
}

impl From<ConfigScope> for Source {
    fn from(scope: ConfigScope) -> Self {
        match scope {
            ConfigScope::Global => Self::Global,
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
    pub(crate) fn apply_to(&self, effective: &mut EffectiveConfig, source: Source) {
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
                    value: gpus.clone(),
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
            if let Some(detach) = submit.detach {
                effective.submit.detach = WithSource {
                    value: detach,
                    source,
                };
            }
            if let Some(gpus) = submit.gpus {
                effective.submit.gpus = WithSource {
                    value: gpus,
                    source,
                };
            }
        }
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
                gpus: Some(effective.start.gpus.value.clone()),
                interval: Some(effective.start.interval.value),
                scheduler: Some(effective.start.scheduler.value),
                threshold: Some(effective.start.threshold.value),
            }),
            submit: Some(SubmitConfig {
                detach: Some(effective.submit.detach.value),
                gpus: Some(effective.submit.gpus.value),
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
    pub(crate) gpus: WithSource<String>,
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
    /// Whether to return without following the submitted job log.
    pub(crate) detach: WithSource<bool>,
    /// Number of GPUs requested by a job.
    pub(crate) gpus: WithSource<usize>,
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
                gpus: WithSource::builtin("all".to_string()),
                interval: WithSource::builtin(10),
                scheduler: WithSource::builtin(SchedulerKind::Greedy),
                threshold: WithSource::builtin(10),
            },
            submit: EffectiveSubmitConfig {
                detach: WithSource::builtin(false),
                gpus: WithSource::builtin(1),
            },
        }
    }
}

/// Configuration file scope.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ConfigScope {
    /// System-wide configuration.
    Global,
    /// Current user's configuration.
    User,
}

impl From<bool> for ConfigScope {
    fn from(global: bool) -> Self {
        if global { Self::Global } else { Self::User }
    }
}
