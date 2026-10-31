//! Job-related types
//!
//! Defines job struct, status enum, request and print info structures.

use std::{collections::HashMap, path::Path};

use comfy_table::Color;
use serde::{Deserialize, Serialize};

use crate::constants::{JOB_LOG_FILE_PREFIX, JOB_LOG_FILE_SUFFIX};

use super::gpu::GpuSelection;

/// Job struct (used for database storage and scheduling)
pub(crate) struct Job {
    /// Job ID
    pub(crate) id: usize,
    /// Job status
    pub(crate) status: JobStatus,
    /// Submitting user
    pub(crate) username: String,
    /// Command
    pub(crate) command: Vec<String>,
    /// GPU selection this job may use.
    pub(crate) allowed_gpus: GpuSelection,
    /// GPU IDs allocated to the job.
    pub(crate) allocated_gpus: Option<Vec<usize>>,
    /// Number of GPUs requested
    pub(crate) required_gpus: usize,
    /// Environment variables snapshot
    pub(crate) envs: HashMap<String, String>,
    /// Log file path
    pub(crate) log_path: String,
    /// Working directory at submission time
    pub(crate) cwd: String,
    /// Number of unfinished prerequisite jobs
    pub(crate) remaining_dependencies: usize,
    /// Job start time (Unix timestamp), None if not started
    pub(crate) start_time: Option<i64>,
    /// Job end time (Unix timestamp), None if not ended
    pub(crate) end_time: Option<i64>,
}

impl Job {
    /// Create job from request
    ///
    /// Resolve custom relative logs against the saved working directory.
    ///
    /// # Arguments
    ///
    /// * `id` - Job ID
    /// * `req` - Job request
    /// * `log_dir` - Selected daemon's default task log directory.
    ///
    /// # Returns
    ///
    /// A pending job with no GPU allocation and a resolved log path.
    pub(crate) fn from_request(id: usize, req: JobRequest, log_dir: &Path) -> Self {
        let log_path = req
            .log_path
            .map(|path| {
                if Path::new(&path).is_absolute() {
                    path
                } else {
                    Path::new(&req.cwd)
                        .join(path)
                        .to_string_lossy()
                        .into_owned()
                }
            })
            .unwrap_or_else(|| {
                log_dir
                    .join(format!("{JOB_LOG_FILE_PREFIX}{id}{JOB_LOG_FILE_SUFFIX}"))
                    .to_string_lossy()
                    .into_owned()
            });
        Job {
            id,
            status: JobStatus::Pending,
            username: req.username,
            command: req.command,
            allowed_gpus: req.allowed_gpus,
            allocated_gpus: None,
            required_gpus: req.required_gpus,
            envs: req.envs,
            log_path,
            cwd: req.cwd,
            remaining_dependencies: 0,
            start_time: None,
            end_time: None,
        }
    }
}

/// Job status enum
#[derive(PartialEq, Serialize, Deserialize, Debug)]
pub(crate) enum JobStatus {
    /// Unknown
    Unknown,
    /// Waiting for prerequisite jobs to complete
    Blocked,
    /// Waiting to be scheduled
    Pending,
    /// Running
    Running,
    /// Completed successfully
    Completed,
    /// Failed
    Failed,
    /// Cancelled
    Cancelled,
}

impl JobStatus {
    /// Convert to string
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Unknown => "unknown",
            JobStatus::Blocked => "blocked",
            JobStatus::Pending => "pending",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }

    /// Parse status from string
    pub(crate) fn from_str(s: &str) -> Self {
        match s {
            "blocked" => JobStatus::Blocked,
            "pending" => JobStatus::Pending,
            "running" => JobStatus::Running,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "cancelled" => JobStatus::Cancelled,
            _ => JobStatus::Unknown,
        }
    }

    /// Get terminal color for status
    pub(crate) fn color(&self) -> Color {
        match self {
            JobStatus::Blocked => Color::Yellow,
            JobStatus::Running => Color::Green,
            JobStatus::Pending => Color::Yellow,
            JobStatus::Failed => Color::Red,
            JobStatus::Completed => Color::Green,
            JobStatus::Cancelled => Color::DarkGrey,
            JobStatus::Unknown => Color::DarkGrey,
        }
    }
}

/// Job request struct (used by client to submit)
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct JobRequest {
    /// Submitting user
    pub(crate) username: String,
    /// GPU selection this job may use.
    pub(crate) allowed_gpus: GpuSelection,
    /// Command
    pub(crate) command: Vec<String>,
    /// Prerequisite job IDs
    pub(crate) dependencies: Vec<usize>,
    /// Number of GPUs requested
    pub(crate) required_gpus: usize,
    /// Environment variables snapshot
    pub(crate) envs: HashMap<String, String>,
    /// Log file path
    pub(crate) log_path: Option<String>,
    /// Working directory at submission time
    pub(crate) cwd: String,
}

/// Job info struct for printing
#[derive(Serialize, Deserialize)]
pub(crate) struct JobPrintInfo {
    /// Job ID
    pub(crate) id: usize,
    /// Job status
    pub(crate) status: JobStatus,
    /// Submitting user
    pub(crate) username: String,
    /// Command
    pub(crate) command: Vec<String>,
    /// GPU selection this job may use.
    pub(crate) allowed_gpus: GpuSelection,
    /// GPU IDs allocated to the job.
    pub(crate) allocated_gpus: Option<Vec<usize>>,
    /// Prerequisite job IDs
    pub(crate) dependencies: Vec<usize>,
    /// Number of GPUs requested
    pub(crate) required_gpus: usize,
    /// Number of unfinished prerequisite jobs
    pub(crate) remaining_dependencies: usize,
    /// Job start time (Unix timestamp), None if not started
    pub(crate) start_time: Option<i64>,
    /// Job end time (Unix timestamp), None if not ended
    pub(crate) end_time: Option<i64>,
}

/// Complete job details.
#[derive(Serialize, Deserialize)]
pub(crate) struct JobInfo {
    /// Job ID.
    pub(crate) id: usize,
    /// Job status.
    pub(crate) status: JobStatus,
    /// Submitting user.
    pub(crate) username: String,
    /// Saved command arguments.
    pub(crate) command: Vec<String>,
    /// GPU selection this job may use.
    pub(crate) allowed_gpus: GpuSelection,
    /// Most recently allocated GPU IDs.
    pub(crate) allocated_gpus: Option<Vec<usize>>,
    /// Number of GPUs requested.
    pub(crate) required_gpus: usize,
    /// Saved environment variables.
    pub(crate) envs: HashMap<String, String>,
    /// Log file path.
    pub(crate) log_path: String,
    /// Saved working directory.
    pub(crate) cwd: String,
    /// Number of unfinished prerequisites.
    pub(crate) remaining_dependencies: usize,
    /// Start time as Unix seconds, if started.
    pub(crate) start_time: Option<i64>,
    /// End time as Unix seconds, if ended.
    pub(crate) end_time: Option<i64>,
    /// Prerequisite job IDs.
    pub(crate) dependencies: Vec<usize>,
}

impl JobInfo {
    /// Combine a persisted job with its prerequisite IDs.
    ///
    /// # Arguments
    ///
    /// * `job` - Persisted job to consume.
    /// * `dependencies` - Prerequisite IDs in ascending order.
    ///
    /// # Returns
    ///
    /// Complete details for a single job.
    pub(crate) fn from_job(job: Job, dependencies: Vec<usize>) -> Self {
        Self {
            id: job.id,
            status: job.status,
            username: job.username,
            command: job.command,
            allowed_gpus: job.allowed_gpus,
            allocated_gpus: job.allocated_gpus,
            required_gpus: job.required_gpus,
            envs: job.envs,
            log_path: job.log_path,
            cwd: job.cwd,
            remaining_dependencies: job.remaining_dependencies,
            start_time: job.start_time,
            end_time: job.end_time,
            dependencies,
        }
    }
}
