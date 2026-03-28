//! Greedy scheduler
//!
//! Iterates all pending jobs each cycle, allocates GPUs to each job in order if available.

use std::sync::Arc;

use crate::{
    daemon::scheduler::{Scheduler, launch_job},
    db::DbPool,
    hardware::GpuPool,
    types::Job,
};

/// Greedy scheduler
pub struct GreedyScheduler;

impl GreedyScheduler {
    /// Create a new Greedy Scheduler instance
    pub fn new() -> Self {
        Self {}
    }
}

impl Scheduler for GreedyScheduler {
    fn schedule(&mut self, pending_jobs: Vec<Job>, db: DbPool, gpu_pool: GpuPool) {
        for job in pending_jobs {
            let available_gpus = {
                let gpu_pool = gpu_pool.lock().unwrap();
                gpu_pool.find_available_gpus(job.gpus)
            };

            if let Ok(gpus) = available_gpus {
                launch_job(job, gpus, Arc::clone(&db), Arc::clone(&gpu_pool));
            }
        }
    }
}
