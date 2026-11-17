//! IPC-related types
//!
//! Defines delete target, request and response structures for daemon IPC communication.

use std::collections::HashMap;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::constants::PROTOCOL_VERSION;

use super::{
    daemon::DaemonInfo,
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
    Stop {
        /// Whether to force the stop request.
        force: bool,
    },
    /// Submit job request
    Submit {
        /// Job request data
        job: JobRequest,
    },
    /// Rerun job request
    Rerun {
        /// Source job ID
        id: usize,
        /// Optional name override.
        name: Option<String>,
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
    },
    /// Cancel job request
    Cancel {
        /// Job ID
        id: usize,
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
        /// Limit number of jobs returned
        limit: Option<usize>,
        /// Optional name substring filter.
        name: Option<String>,
    },
    /// Get complete job details after checking ownership.
    Info {
        /// Job ID.
        id: usize,
    },
    /// Get job log path request
    Log {
        /// Job ID
        id: usize,
    },
}

/// Daemon response enum
#[derive(Serialize, Deserialize)]
pub(crate) enum Response {
    /// Stop check response
    Stop,
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
        /// Basic daemon metadata.
        daemon: DaemonInfo,
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

/// Versioned socket protocol message.
#[derive(Serialize, Deserialize)]
pub(crate) struct Envelope<T> {
    /// Socket protocol version.
    version: u32,
    /// Business payload.
    pub(crate) payload: T,
}

impl<T> Envelope<T> {
    /// Wrap a payload using the current protocol version.
    ///
    /// # Arguments
    ///
    /// * `payload` - The business payload to wrap.
    ///
    /// # Returns
    ///
    /// A new `Envelope` instance containing the payload and protocol version.
    pub(crate) fn new(payload: T) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            payload,
        }
    }
}

impl<T: DeserializeOwned> Envelope<T> {
    /// Parse a versioned message.
    ///
    /// # Arguments
    ///
    /// * `message` - The JSON message to parse.
    ///
    /// # Returns
    ///
    /// The parsed payload or an error message.
    pub(crate) fn parse(message: &str) -> Result<T, String> {
        #[derive(Deserialize)]
        struct Version {
            version: u32,
        }

        let version: Version = serde_json::from_str(message)
            .map_err(|error| format!("Invalid protocol message: {error}"))?;
        if version.version != PROTOCOL_VERSION {
            return Err(format!(
                "Incompatible protocol version: expected {PROTOCOL_VERSION}, received {}",
                version.version
            ));
        }
        let envelope: Self = serde_json::from_str(message)
            .map_err(|error| format!("Failed to parse protocol message: {error}"))?;

        Ok(envelope.payload)
    }
}

#[cfg(test)]
mod tests {
    use crate::constants::PROTOCOL_VERSION;

    use super::{Envelope, Request, Response};

    #[test]
    fn versioned_messages_round_trip() {
        let request = serde_json::to_string(&Envelope::new(Request::Status)).unwrap();
        assert!(matches!(
            Envelope::<Request>::parse(&request).unwrap(),
            Request::Status
        ));

        let response = serde_json::to_string(&Envelope::new(Response::Stop)).unwrap();
        assert!(matches!(
            Envelope::<Response>::parse(&response).unwrap(),
            Response::Stop
        ));
    }

    #[test]
    fn rejects_invalid_and_incompatible_messages() {
        let incompatible = format!(
            r#"{{"version":{},"payload":{{"unknown":true}}}}"#,
            PROTOCOL_VERSION + 1
        );
        assert!(
            Envelope::<Request>::parse(&incompatible)
                .unwrap_err()
                .contains("Incompatible")
        );
        for message in ["{", r#"{"payload":"Status"}"#, r#"{"version":"1"}"#] {
            assert!(Envelope::<Request>::parse(message).is_err());
        }
        let unknown = format!(r#"{{"version":{PROTOCOL_VERSION},"payload":"Unknown"}}"#);
        assert!(Envelope::<Request>::parse(&unknown).is_err());
    }
}
