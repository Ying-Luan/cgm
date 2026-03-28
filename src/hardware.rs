//! Hardware module
//!
//! Provides GPU management functionality.

mod gpu;

pub(crate) use gpu::{GpuManager, GpuPool, get_gpu_info};
