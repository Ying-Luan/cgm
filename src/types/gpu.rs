//! GPU-related types
//!
//! Defines GPU state, info and print info structures.

use serde::{Deserialize, Serialize};

/// GPU state enum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GpuState {
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
pub struct GpuInfo {
    /// GPU index
    pub id: usize,
    /// GPU state
    pub state: GpuState,
    /// Process PID using this GPU
    pub pid: Option<u32>,
    /// Job ID using this GPU
    pub job_id: Option<usize>,
}

/// GPU info struct for printing
pub struct GpuPrintInfo {
    /// GPU index
    pub id: usize,
    /// GPU name
    pub name: String,
    /// Memory used (bytes)
    pub memory_used: u64,
    /// Total memory (bytes)
    pub memory_total: u64,
    /// Temperature (celsius)
    pub temp: u32,
    /// GPU utilization (%)
    pub util_gpu: u32,
    /// Memory bandwidth utilization (%)
    pub util_memory: u32,
}
