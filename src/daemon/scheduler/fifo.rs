//! FIFO Scheduler
//!
//! Iterates all pending jobs each cycle, reserves GPUs for each job in order if available, and launches when enough are reserved.

use std::collections::{HashMap, HashSet};

use crate::{
    daemon::scheduler::Scheduler,
    hardware::GpuPool,
    types::{GpuState, Job},
};

/// First-In-First-Out (FIFO) Scheduler
pub(crate) struct FifoScheduler {
    /// A map of reserved GPUs
    ///
    /// gpu_id -> job_id, tracks logically reserved but not yet launched GPUs
    reserved: HashMap<usize, usize>,
}

impl FifoScheduler {
    /// Create a new FIFO Scheduler instance
    pub(crate) fn new() -> Self {
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
    fn schedule(
        &mut self,
        gpu_pool: GpuPool,
        launch: &mut dyn FnMut(Job, Vec<usize>),
        pending_jobs: Vec<Job>,
    ) {
        // Clean up reservations for jobs no longer pending or for gpus no longer idle.
        let pending_ids: HashSet<usize> = pending_jobs.iter().map(|j| j.id).collect();
        let mut idle = gpu_pool
            .lock()
            .unwrap()
            .find_gpu_ids_by_state(GpuState::Idle);
        self.reserved
            .retain(|gpu_id, job_id| pending_ids.contains(job_id) && idle.contains(gpu_id));
        idle.retain(|id| !self.reserved.contains_key(id));

        for job in pending_jobs {
            let already_reserved = self.reserved_for_job(job.id);
            let available: Vec<usize> = idle
                .iter()
                .filter(|id| job.allowed_gpus.allows(**id))
                .copied()
                .collect();
            let needed = job.required_gpus;
            let total_available = already_reserved.len() + available.len();

            if total_available >= needed {
                // Enough GPUs: combine reserved + idle, take exactly what we need
                let mut gpus = already_reserved.clone();
                let extra_needed = needed - gpus.len();
                let extra: Vec<usize> = available.into_iter().take(extra_needed).collect();
                gpus.extend(&extra);

                // Remove used idle GPUs from idle list for subsequent jobs
                idle.retain(|id| !extra.contains(id));

                // Clear reservation for this job before launching
                self.clear_job_reservation(job.id);

                launch(job, gpus);
            } else {
                // Reserve only GPUs this job may use, leaving unrelated GPUs available.
                self.reserve(job.id, &available);
                idle.retain(|id| !available.contains(id));
                // Continue iterating: other jobs may have their own reserved GPUs
            }
        }
    }
}
