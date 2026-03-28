//! Scheduler related types.
//!
//! Defines scheduler kind enum.

use clap::ValueEnum;

/// Scheduler kind enum
#[derive(ValueEnum, Clone, Debug)]
pub enum SchedulerKind {
    /// Fifo scheduler (first-in-first-out)
    Fifo,
    /// Greedy scheduler
    Greedy,
}
