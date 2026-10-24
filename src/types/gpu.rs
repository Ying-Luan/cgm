//! GPU-related types
//!
//! Defines GPU state, info and print info structures.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::utils::parse_comma_separated_ids;

/// GPU selection for a daemon or job.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GpuSelection {
    /// All managed GPUs.
    All,
    /// A specific set of GPU IDs.
    Allowed(Vec<usize>),
}

impl GpuSelection {
    /// Parse `all` or a comma-separated GPU ID list.
    ///
    /// # Arguments
    ///
    /// * `value` - GPU selection text, either `all` or comma-separated IDs.
    ///
    /// # Returns
    ///
    /// The parsed GPU selection, or an error message when the value contains
    /// an empty, invalid, or duplicate GPU ID.
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        if value == "all" {
            return Ok(Self::All);
        }

        parse_comma_separated_ids(Some(value)).map(|ids| Self::Allowed(ids.unwrap_or_default()))
    }

    /// Check whether a GPU ID is allowed by this selection.
    ///
    /// # Arguments
    ///
    /// * `gpu_id` - GPU ID to check.
    ///
    /// # Returns
    ///
    /// Whether the GPU ID is allowed by this selection.
    pub(crate) fn allows(&self, gpu_id: usize) -> bool {
        match self {
            Self::All => true,
            Self::Allowed(ids) => ids.contains(&gpu_id),
        }
    }

    /// Return the number of allowed GPUs when the selection is bounded.
    ///
    /// # Returns
    ///
    /// The number of allowed GPU IDs, or `None` when all GPUs are allowed.
    pub(crate) fn len(&self) -> Option<usize> {
        match self {
            Self::All => None,
            Self::Allowed(ids) => Some(ids.len()),
        }
    }

    /// Return the selected GPU IDs when the selection is bounded.
    ///
    /// # Returns
    ///
    /// The selected GPU IDs, or `None` when all GPUs are allowed.
    pub(crate) fn ids(&self) -> Option<&[usize]> {
        match self {
            Self::All => None,
            Self::Allowed(ids) => Some(ids),
        }
    }
}

impl fmt::Display for GpuSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => write!(formatter, "all"),
            Self::Allowed(ids) => write!(
                formatter,
                "{}",
                ids.iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

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
