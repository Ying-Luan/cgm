//! IPC-related types
//!
//! Defines delete target, request and response structures for daemon IPC communication.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{
    gpu::{GpuInfo, GpuSelection},
    job::{JobInfo, JobPrintInfo, JobRequest, JobStatus},
};

/// Current terminal values optionally replacing a source job's saved context.
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct RerunContext {
    /// Working directory override; None preserves the saved directory.
    pub(crate) cwd: Option<String>,
    /// Environment override; None preserves the saved environment variables.
    pub(crate) envs: Option<HashMap<String, String>>,
}

/// Delete target enum
#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum DeleteTarget {
    /// Delete single job by ID
    Single(usize),
    /// Delete jobs by statuses
    ByStatuses(Vec<JobStatus>),
    /// Delete all terminated jobs
    AllTerminated,
}

/// Daemon request enum
#[derive(Serialize, Deserialize, Debug)]
pub(crate) enum Request {
    /// Check if stop is safe request
    StopCheck,
    /// Submit job request
    Submit {
        /// Job request data
        job: JobRequest,
    },
    /// Rerun job request
    Rerun {
        /// Source job ID
        id: usize,
        /// Optional current terminal values overriding the saved context
        context: RerunContext,
        /// Optional allowed GPU selection override
        allowed_gpus: Option<GpuSelection>,
        /// Optional dependency override
        dependencies: Option<Vec<usize>>,
        /// Optional required GPU count override
        required_gpus: Option<usize>,
        /// Optional log path override
        log_path: Option<String>,
        /// Requesting username
        username: String,
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
    /// Get complete job details after checking ownership.
    Info {
        /// Job ID.
        id: usize,
        /// Requesting username.
        username: String,
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
pub(crate) enum Response {
    /// Stop check response
    StopCheck {
        /// Number of running jobs
        running_count: usize,
    },
    /// Submit job success response
    Submit {
        /// Persisted details of the newly submitted job.
        job: Box<JobInfo>,
    },
    /// Rerun job success response
    Rerun {
        /// Persisted details of the new job, not the source job.
        job: Box<JobInfo>,
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
    /// Complete job details response.
    Info {
        /// Persisted job details and prerequisite IDs.
        job: Box<JobInfo>,
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
