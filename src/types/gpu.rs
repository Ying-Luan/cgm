//! GPU-related types
//!
//! Defines GPU state, info and print info structures.

use serde::{Deserialize, Serialize};

/// GPU state enum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum GpuState {
    /// Idle, available for allocation
    Idle,
    /// In use by a CGM job
    InUse,
    /// Occupied by external process
    External,
    /// GPU error
    Error,
}

/// GPU info struct
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct GpuInfo {
    /// GPU index
    pub(crate) id: usize,
    /// GPU state
    pub(crate) state: GpuState,
    /// Process PID using this GPU
    pub(crate) pid: Option<u32>,
    /// Job ID using this GPU
    pub(crate) job_id: Option<usize>,
}

/// GPU info struct for printing
pub(crate) struct GpuPrintInfo {
    /// GPU index
    pub(crate) id: usize,
    /// GPU name
    pub(crate) name: String,
    /// Memory used (bytes)
    pub(crate) memory_used: u64,
    /// Total memory (bytes)
    pub(crate) memory_total: u64,
    /// Temperature (celsius)
    pub(crate) temp: u32,
    /// GPU utilization (%)
    pub(crate) util_gpu: u32,
    /// Memory bandwidth utilization (%)
    pub(crate) util_memory: u32,
}
