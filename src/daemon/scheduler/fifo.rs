//! FIFO Scheduler
//!
//! Iterates all pending jobs each cycle, reserves GPUs for each job in order if available, and launches when enough are reserved.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    daemon::scheduler::{Scheduler, launch_job},
    db::DbPool,
    hardware::GpuPool,
    types::{GpuState, Job},
};

/// First-In-First-Out (FIFO) Scheduler
pub struct FifoScheduler {
    /// A map of reserved GPUs
    ///
    /// gpu_id -> job_id, tracks logically reserved but not yet launched GPUs
    reserved: HashMap<usize, usize>,
}

impl FifoScheduler {
    /// Create a new FIFO Scheduler instance
    pub fn new() -> Self {
        Self {
            reserved: HashMap::new(),
        }
    }

    /// Get reserved gpu_ids for a specific job
    ///
    /// # Arguments
    ///
    /// * `job_id` - The ID of the job to check reservations for
    ///
    /// # Returns
    ///
    /// A vector of GPU IDs that are reserved for the specified job
    fn reserved_for_job(&self, job_id: usize) -> Vec<usize> {
        self.reserved
            .iter()
            .filter_map(|(&gpu_id, &j_id)| if j_id == job_id { Some(gpu_id) } else { None })
            .collect()
    }

    /// Remove all reservations for a specific job
    ///
    /// # Arguments
    ///
    /// * `job_id` - The ID of the job to clear reservations for
    fn clear_job_reservation(&mut self, job_id: usize) {
        self.reserved.retain(|_, &mut jid| jid != job_id);
    }

    /// Reserve gpu_ids for a job (insert into reserved, mark as logically occupied)
    ///
    /// # Arguments
    ///
    /// * `job_id` - The ID of the job to reserve GPUs for
    /// * `gpu_ids` - A slice of GPU IDs to reserve
    fn reserve(&mut self, job_id: usize, gpu_ids: &[usize]) {
        for &gpu_id in gpu_ids {
            self.reserved.insert(gpu_id, job_id);
        }
    }
}

impl Scheduler for FifoScheduler {
    fn schedule(&mut self, pending_jobs: Vec<Job>, db: DbPool, gpu_pool: GpuPool) {
        // Clean up reservations for jobs no longer in pending list
        let pending_ids: HashSet<usize> = pending_jobs.iter().map(|j| j.id).collect();
        self.reserved.retain(|_, jid| pending_ids.contains(jid));

        // get all idle GPU ids and filter out reserved ones
        let mut idle = gpu_pool
            .lock()
            .unwrap()
            .find_gpu_ids_by_state(GpuState::Idle);
        idle.retain(|id| !self.reserved.contains_key(id));

        for job in pending_jobs {
            let already_reserved = self.reserved_for_job(job.id);
            let needed = job.gpus;
            let total_available = already_reserved.len() + idle.len();

            if total_available >= needed {
                // Enough GPUs: combine reserved + idle, take exactly what we need
                let mut gpus = already_reserved.clone();
                let extra_needed = needed - gpus.len();
                let extra: Vec<usize> = idle.iter().take(extra_needed).copied().collect();
                gpus.extend(&extra);

                // Remove used idle GPUs from idle list for subsequent jobs
                idle.retain(|id| !extra.contains(id));

                // Clear reservation for this job before launching
                self.clear_job_reservation(job.id);

                launch_job(job, gpus, Arc::clone(&db), Arc::clone(&gpu_pool));
            } else {
                // Not enough: absorb remaining idle into reservation for this job
                self.reserve(job.id, &idle);
                idle.clear();
                // Continue iterating: other jobs may have their own reserved GPUs
            }
        }
    }
}
