//! Job scheduling strategies
//!
//! Select pending jobs and GPUs; execution and lifecycle belong to the executor.

mod fifo;
mod greedy;

pub(crate) use fifo::FifoScheduler;
pub(crate) use greedy::GreedyScheduler;

use crate::{hardware::GpuPool, types::Job};

/// Scheduler trait, implemented by different scheduling algorithms
pub(crate) trait Scheduler {
    /// Schedule pending jobs, allocate GPUs and launch subprocesses
    ///
    /// # Arguments
    ///
    /// * `gpu_pool` - GPU manager connection pool for allocating and releasing GPUs
    /// * `launch` - Callback that launches a job with its allocated GPUs
    /// * `pending_jobs` - List of pending jobs to schedule
    fn schedule(
        &mut self,
        gpu_pool: GpuPool,
        launch: &mut dyn FnMut(Job, Vec<usize>),
        pending_jobs: Vec<Job>,
    );
}
