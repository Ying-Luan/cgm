//! Job-related types
//!
//! Defines job struct, status enum, request and print info structures.

use std::collections::HashMap;

use comfy_table::Color;
use serde::{Deserialize, Serialize};

use crate::constants::JOB_LOG_FOLDER;

/// Job struct (used for database storage and scheduling)
pub struct Job {
    /// Job ID
    pub id: usize,
    /// Job status
    pub status: JobStatus,
    /// Submitting user
    pub username: String,
    /// Command
    pub command: Vec<String>,
    /// Number of GPUs requested
    pub gpus: usize,
    /// Environment variables snapshot
    pub envs: HashMap<String, String>,
    /// Log file path
    pub log_path: String,
    /// Working directory at submission time
    pub cwd: String,
    /// Job start time (Unix timestamp), None if not started
    pub start_time: Option<i64>,
    /// Job end time (Unix timestamp), None if not ended
    pub end_time: Option<i64>,
}

impl Job {
    /// Create job from request
    ///
    /// # Arguments
    ///
    /// * `id` - Job ID
    /// * `req` - Job request
    pub fn from_request(id: usize, req: JobRequest) -> Self {
        let log_path = req
            .log_path
            .unwrap_or_else(|| format!("{}/job-{}.log", JOB_LOG_FOLDER, id));
        Job {
            id,
            status: JobStatus::Pending,
            username: req.username,
            command: req.command,
            gpus: req.gpus,
            envs: req.envs,
            log_path,
            cwd: req.cwd,
            start_time: None,
            end_time: None,
        }
    }
}

/// Job status enum
#[derive(PartialEq, Serialize, Deserialize, Debug)]
pub enum JobStatus {
    /// Unknown
    Unknown,
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
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Unknown => "unknown",
            JobStatus::Pending => "pending",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }

    /// Parse status from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => JobStatus::Pending,
            "running" => JobStatus::Running,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "cancelled" => JobStatus::Cancelled,
            _ => JobStatus::Unknown,
        }
    }

    /// Get terminal color for status
    pub fn color(&self) -> Color {
        match self {
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
pub struct JobRequest {
    /// Submitting user
    pub username: String,
    /// Command
    pub command: Vec<String>,
    /// Number of GPUs requested
    pub gpus: usize,
    /// Environment variables snapshot
    pub envs: HashMap<String, String>,
    /// Log file path
    pub log_path: Option<String>,
    /// Working directory at submission time
    pub cwd: String,
}

/// Job info struct for printing
#[derive(Serialize, Deserialize)]
pub struct JobPrintInfo {
    /// Job ID
    pub id: usize,
    /// Job status
    pub status: JobStatus,
    /// Submitting user
    pub username: String,
    /// Command
    pub command: Vec<String>,
    /// Number of GPUs requested
    pub gpus: usize,
    /// Job start time (Unix timestamp), None if not started
    pub start_time: Option<i64>,
    /// Job end time (Unix timestamp), None if not ended
    pub end_time: Option<i64>,
}
