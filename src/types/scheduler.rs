//! Scheduler related types.
//!
//! Defines scheduler kind enum.

use std::fmt;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Scheduler kind enum
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SchedulerKind {
    /// Fifo scheduler (first-in-first-out)
    Fifo,
    /// Greedy scheduler
    Greedy,
}

impl fmt::Display for SchedulerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fifo => write!(f, "fifo"),
            Self::Greedy => write!(f, "greedy"),
        }
    }
}
