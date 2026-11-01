//! Daemon runtime context, state paths, peer authorization, and the abstract socket.

mod identity;
mod paths;
pub(crate) mod socket;

pub(crate) use identity::{authorize, connection_uid};
pub(crate) use paths::RuntimePaths;

use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
};

use nix::unistd::{Uid, User};

use crate::types::{DaemonInfo, DaemonMode, DaemonState};

use self::paths::secure_dir;

/// Startup-selected mode, owner identity, and paths for one daemon.
pub(crate) struct Runtime {
    /// Explicit mode, never inferred from account names.
    pub(crate) mode: DaemonMode,
    /// Effective owner UID.
    pub(crate) owner_uid: u32,
    /// Resolved owner account name.
    pub(crate) owner_name: String,
    /// Executable path captured before daemonization.
    pub(crate) executable: PathBuf,
    /// Persistent state and log paths.
    pub(crate) paths: RuntimePaths,
}

impl Runtime {
    /// Resolve the startup identity and paths for the selected mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Explicit operating mode selected by the start command.
    ///
    /// # Returns
    ///
    /// Owner metadata, executable path, and selected state paths without creating files.
    pub(crate) fn new(mode: DaemonMode) -> Result<Self, String> {
        let uid = Uid::effective();
        let owner = User::from_uid(uid)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Current user not found".to_string())?;
        Ok(Self {
            mode,
            owner_uid: uid.as_raw(),
            owner_name: owner.name,
            executable: std::env::current_exe().map_err(|error| error.to_string())?,
            paths: RuntimePaths::select(mode == DaemonMode::User)?,
        })
    }

    /// Create and secure state directories and existing state files.
    ///
    /// Sets the process umask to `0077`; existing database and daemon log files
    /// must be regular, singly linked files owned by the effective user.
    ///
    /// # Returns
    ///
    /// Success after securing directories and setting existing state files to `0600`.
    pub(crate) fn secure(&self) -> Result<(), String> {
        let paths = &self.paths;
        let mode = match self.mode {
            DaemonMode::User => 0o700,
            DaemonMode::System => 0o711,
        };
        secure_dir(&paths.root, mode)?;
        secure_dir(&paths.job_logs, mode)?;
        // SQLite and daemon output must remain private under a traversable system directory.
        unsafe {
            nix::libc::umask(0o077);
        }
        let sqlite = paths.sqlite_files();
        for path in sqlite
            .iter()
            .map(PathBuf::as_path)
            .chain([paths.stdout.as_path(), paths.stderr.as_path()])
        {
            match fs::symlink_metadata(path) {
                Ok(metadata)
                    if metadata.is_file()
                        && metadata.uid() == Uid::effective().as_raw()
                        && metadata.nlink() == 1 =>
                {
                    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                        .map_err(|e| format!("Failed to secure {}: {}", path.display(), e))?;
                }
                Ok(_) => return Err(format!("Unsafe state file: {}", path.display())),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("Failed to inspect {}: {}", path.display(), e)),
            }
        }

        Ok(())
    }

    /// Snapshot basic metadata in the daemon process.
    ///
    /// # Arguments
    ///
    /// * `state` - Runtime state to report in the snapshot.
    ///
    /// # Returns
    ///
    /// Metadata with this process's PID and the startup-selected identity and paths.
    pub(crate) fn info(&self, state: DaemonState) -> DaemonInfo {
        DaemonInfo {
            state,
            mode: self.mode,
            owner_uid: self.owner_uid,
            owner_name: self.owner_name.clone(),
            pid: std::process::id(),
            version: env!("CARGO_PKG_VERSION").into(),
            executable: self.executable.clone(),
            root: self.paths.root.clone(),
        }
    }
}
