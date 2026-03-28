//! Shared type definitions
//!
//! Defines GPU state, job status, request/response and other core data structures.

mod gpu;
mod ipc;
mod job;
mod scheduler;

pub(crate) use gpu::{GpuInfo, GpuPrintInfo, GpuState};
pub(crate) use ipc::{DeleteTarget, Request, Response};
pub(crate) use job::{Job, JobPrintInfo, JobRequest, JobStatus};
pub(crate) use scheduler::SchedulerKind;
