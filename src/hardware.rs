//! Hardware module
//!
//! Provides GPU management functionality.

mod gpu;

pub use gpu::{GpuManager, GpuPool, get_gpu_info};
