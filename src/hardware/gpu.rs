//! GPU management module
//!
//! Provides GpuManager struct for GPU state management, allocation and release.

use std::sync::{Arc, Mutex};

use nvml_wrapper::Nvml;

use crate::{
    macros::red,
    types::{GpuInfo, GpuPrintInfo, GpuState},
};

/// GPU manager struct
pub struct GpuManager {
    /// GPU info list
    gpus: Vec<GpuInfo>,
    /// GPU memory usage threshold, above this value considered externally occupied
    threshold: u32,
    /// NVML instance
    nvml: Nvml,
}

/// GPU manager connection pool type alias
pub type GpuPool = Arc<Mutex<GpuManager>>;

impl GpuManager {
    /// Create GpuManager instance
    ///
    /// # Arguments
    ///
    /// * `gpu_indices` - GPU index list managed by CGM
    /// * `threshold` - GPU memory threshold (%)
    pub fn new(gpu_indices: Vec<usize>, threshold: u32) -> GpuPool {
        let nvml = Nvml::init().expect("Failed to initialize NVML");
        let device_count = nvml
            .device_count()
            .expect("Failed to query NVML device count") as usize;

        let gpus: Vec<GpuInfo> = gpu_indices
            .into_iter()
            .filter(|&id| id < device_count)
            .map(|id| GpuInfo {
                id,
                state: GpuState::Idle,
                pid: None,
                job_id: None,
            })
            .collect();

        Arc::new(Mutex::new(GpuManager {
            gpus,
            threshold,
            nvml,
        }))
    }

    /// Batch get GPU memory utilization
    ///
    /// # Arguments
    ///
    /// * `indices` - GPU index list
    ///
    /// # Returns
    ///
    /// Each GPU's memory utilization (percentage), returns 100 on failure
    fn get_gpu_util(&self, indices: &[usize]) -> Vec<u32> {
        indices
            .iter()
            .map(|&i| {
                // Get GPU memory utilization via NVML, returns 100 on failure
                match self
                    .nvml
                    .device_by_index(i as u32)
                    .and_then(|d| d.memory_info())
                {
                    Ok(m) => (m.used as f64 / m.total as f64 * 100.0) as u32,
                    Err(e) => {
                        eprintln!("{}", red!("Failed to get memory info for GPU {}: {}", i, e));
                        100
                    }
                }
            })
            .collect()
    }

    /// Get mutable reference by GPU id
    ///
    /// # Arguments
    ///
    /// * `id` - GPU id
    ///
    /// # Returns
    ///
    /// GPU mutable reference, None if not found
    fn get_gpu_mut(&mut self, id: usize) -> Option<&mut GpuInfo> {
        self.gpus.iter_mut().find(|g| g.id == id)
    }

    /// Allocate single GPU to job
    ///
    /// # Arguments
    ///
    /// * `idx` - GPU index
    /// * `job_id` - Job ID
    fn allocate(&mut self, idx: usize, job_id: usize) {
        if let Some(gpu) = self.get_gpu_mut(idx) {
            gpu.state = GpuState::InUse;
            gpu.job_id = Some(job_id);
        }
    }

    /// Release single GPU
    ///
    /// # Arguments
    ///
    /// * `idx` - GPU index
    fn release(&mut self, idx: usize) {
        if let Some(gpu) = self.get_gpu_mut(idx) {
            gpu.state = GpuState::Idle;
            gpu.job_id = None;
            gpu.pid = None;
        }
    }

    /// Update single GPU's process PID
    ///
    /// # Arguments
    ///
    /// * `idx` - GPU index
    /// * `pid` - Process ID
    fn update_pid(&mut self, idx: usize, pid: u32) {
        if let Some(gpu) = self.get_gpu_mut(idx) {
            gpu.pid = Some(pid);
        }
    }

    /// Batch update GPU process PID
    ///
    /// # Arguments
    ///
    /// * `gpus` - GPU index list
    /// * `pid` - Process ID
    pub fn update_pid_batch(&mut self, gpus: &[usize], pid: u32) {
        for &idx in gpus {
            self.update_pid(idx, pid);
        }
    }

    /// Get all GPU status
    ///
    /// # Returns
    ///
    /// Cloned copy of GPU info list
    pub fn get_all_status(&self) -> Vec<GpuInfo> {
        self.gpus.clone()
    }

    /// Refresh GPU state
    ///
    /// Checks Idle and External GPU memory utilization. External GPUs with
    /// no job assigned can be restored to Idle if memory drops below threshold.
    pub fn refresh_state(&mut self) {
        let target_gpu_ids: Vec<usize> = self
            .gpus
            .iter()
            .filter(|g| matches!(g.state, GpuState::Idle | GpuState::External))
            .map(|g| g.id)
            .collect();

        let utils = self.get_gpu_util(&target_gpu_ids);

        let threshold = self.threshold;
        for (i, &id) in target_gpu_ids.iter().enumerate() {
            if let Some(gpu) = self.get_gpu_mut(id) {
                gpu.state = if utils[i] >= threshold {
                    GpuState::External
                } else {
                    GpuState::Idle
                };
            }
        }
    }

    /// Find GPU ids by state
    ///
    /// # Arguments
    ///
    /// * `state` - GPU state to filter
    ///
    /// # Returns
    ///
    /// GPU index list with the specified state
    pub fn find_gpu_ids_by_state(&self, state: GpuState) -> Vec<usize> {
        self.gpus
            .iter()
            .filter(|g| g.state == state)
            .map(|g| g.id)
            .collect()
    }

    /// Find available GPUs
    ///
    /// `refresh_state()` should be called before this to get latest GPU state and detect external occupation
    ///
    /// # Arguments
    ///
    /// * `count` - Number of GPUs needed
    ///
    /// # Returns
    ///
    /// Available GPU index list, error if not enough GPUs
    pub fn find_available_gpus(&self, count: usize) -> Result<Vec<usize>, String> {
        // Get all Idle GPU indices
        let available: Vec<usize> = self.find_gpu_ids_by_state(GpuState::Idle);

        if available.len() < count {
            Err(format!(
                "Need {} GPUs but only {} available",
                count,
                available.len()
            ))
        } else {
            Ok(available.into_iter().take(count).collect())
        }
    }

    /// Batch allocate GPUs to job
    ///
    /// # Arguments
    ///
    /// * `job_id` - Job ID
    /// * `gpus` - GPU index list
    pub fn allocate_batch(&mut self, job_id: usize, gpus: &[usize]) {
        for &idx in gpus {
            self.allocate(idx, job_id);
        }

        println!("Allocated GPUs {:?} to job {}", gpus, job_id);
    }

    /// Batch release GPUs
    ///
    /// # Arguments
    ///
    /// * `gpus` - GPU index list
    pub fn release_batch(&mut self, gpus: &[usize]) {
        for &idx in gpus {
            self.release(idx);
        }

        println!("Released GPUs {:?}", gpus);
    }

    /// Get PID of first GPU with this job_id
    ///
    /// # Arguments
    ///
    /// * `job_id` - Job ID
    ///
    /// # Returns
    ///
    /// PID of first GPU with this job_id, None if not found
    pub fn get_first_pid_by_job_id(&self, job_id: usize) -> Option<u32> {
        self.gpus
            .iter()
            .find(|g| g.job_id == Some(job_id))
            .and_then(|g| g.pid)
    }
}

/// Get GPU info list
///
/// # Returns
///
/// GPU info list, empty vector on failure
pub fn get_gpu_info() -> Vec<GpuPrintInfo> {
    let nvml = match Nvml::init() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("{}", red!("Failed to initialize NVML: {}", e));
            return vec![];
        }
    };

    let device_count = nvml.device_count().unwrap_or(0) as usize;
    if device_count == 0 {
        return vec![];
    }

    (0..device_count)
        .filter_map(|i| {
            let device = nvml.device_by_index(i as u32).ok()?;
            let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
            let memory_info = device.memory_info().ok()?;
            let temp = device
                .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
                .ok()
                .unwrap_or(0);
            let util = device.utilization_rates().ok()?;

            Some(GpuPrintInfo {
                id: i,
                name,
                memory_used: memory_info.used,
                memory_total: memory_info.total,
                temp,
                util_gpu: util.gpu,
                util_memory: util.memory,
            })
        })
        .collect()
}
