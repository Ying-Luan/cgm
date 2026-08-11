//! Greedy scheduler
//!
//! Iterates all pending jobs each cycle, allocates GPUs to each job in order if available.

use crate::{daemon::scheduler::Scheduler, hardware::GpuPool, types::Job};

/// Greedy scheduler
pub(crate) struct GreedyScheduler;

impl GreedyScheduler {
    /// Create a new Greedy Scheduler instance
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Scheduler for GreedyScheduler {
    fn schedule(
        &mut self,
        gpu_pool: GpuPool,
        launch: &mut dyn FnMut(Job, Vec<usize>),
        pending_jobs: Vec<Job>,
    ) {
        for job in pending_jobs {
            let available_gpus = {
                let gpu_pool = gpu_pool.lock().unwrap();
                gpu_pool.find_available_gpus(job.allowed_gpus.as_deref(), job.required_gpus)
            };

            if let Ok(gpus) = available_gpus {
                launch(job, gpus);
            }
        }
    }
}
