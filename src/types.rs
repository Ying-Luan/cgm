//! Shared type definitions
//!
//! Defines GPU state, job status, request/response and other core data structures.

mod gpu;
mod ipc;
mod job;
mod scheduler;

pub use gpu::{GpuInfo, GpuPrintInfo, GpuState};
pub use ipc::{DeleteTarget, Request, Response};
pub use job::{Job, JobPrintInfo, JobRequest, JobStatus};
pub use scheduler::SchedulerKind;
