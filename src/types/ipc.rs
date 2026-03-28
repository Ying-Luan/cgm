//! IPC-related types
//!
//! Defines delete target, request and response structures for daemon IPC communication.

use serde::{Deserialize, Serialize};

use crate::types::{
    gpu::GpuInfo,
    job::{JobPrintInfo, JobRequest, JobStatus},
};

/// Delete target enum
#[derive(Serialize, Deserialize, Debug)]
pub enum DeleteTarget {
    /// Delete single job by ID
    Single(usize),
    /// Delete jobs by statuses
    ByStatuses(Vec<JobStatus>),
    /// Delete all terminated jobs
    AllTerminated,
}

/// Daemon request enum
#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    /// Check if stop is safe request
    StopCheck,
    /// Submit job request
    Submit {
        /// Job request data
        job: JobRequest,
    },
    /// Cancel job request
    Cancel {
        /// Job ID
        id: usize,
        /// Username
        username: String,
        /// Force termination
        force: bool,
    },
    /// Delete job request
    Delete {
        /// Delete target
        target: DeleteTarget,
    },
    /// Query GPU status request
    Status,
    /// List all jobs request
    List {
        /// Username
        username: String,
        /// Limit number of jobs returned
        limit: Option<usize>,
    },
    /// Get job log path request
    Log {
        /// Job ID
        id: usize,
        /// Username
        username: String,
    },
}

/// Daemon response enum
#[derive(Serialize, Deserialize)]
pub enum Response {
    /// Stop check response
    StopCheck {
        /// Number of running jobs
        running_count: usize,
    },
    /// Submit job success response
    Submit {
        /// Job ID
        id: usize,
        /// Log file path
        log_path: String,
    },
    /// Cancel job response
    Cancel,
    /// Delete job response
    Delete {
        /// Number of jobs deleted
        count: usize,
    },
    /// Query GPU status response
    Status {
        /// GPU info list
        gpus: Vec<GpuInfo>,
    },
    /// List all jobs response
    List {
        /// Job info list
        jobs: Vec<JobPrintInfo>,
        /// Total job count
        total: usize,
    },
    /// Log path response
    Log {
        /// Log file path
        log_path: String,
    },
    /// Error response
    Error {
        /// Error message
        message: String,
    },
}
