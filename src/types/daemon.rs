//! Daemon identity and reported runtime state.

use std::{fmt, path::PathBuf};

use serde::{Deserialize, Serialize};

/// Explicit daemon operating mode, selected at startup.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) enum DaemonMode {
    /// Personal daemon.
    User,
    /// Machine-wide daemon.
    System,
}

impl fmt::Display for DaemonMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::User => "user",
            Self::System => "system",
        })
    }
}

/// Runtime state.
#[derive(Serialize, Deserialize)]
pub(crate) enum DaemonState {
    /// Serving requests.
    Running,
    /// Coordinated shutdown is in progress.
    Stopping,
}

impl fmt::Display for DaemonState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Running => "Running",
            Self::Stopping => "Stopping",
        })
    }
}

/// Basic metadata available to status callers regardless of their UID.
#[derive(Serialize, Deserialize)]
pub(crate) struct DaemonInfo {
    /// Current runtime state.
    pub(crate) state: DaemonState,
    /// Selected operating mode.
    pub(crate) mode: DaemonMode,
    /// Effective owner UID.
    pub(crate) owner_uid: u32,
    /// Resolved owner account name.
    pub(crate) owner_name: String,
    /// Daemon process ID.
    pub(crate) pid: u32,
    /// Daemon build version.
    pub(crate) version: String,
    /// Executable used to start the daemon.
    pub(crate) executable: PathBuf,
    /// Persistent data directory.
    pub(crate) root: PathBuf,
}
