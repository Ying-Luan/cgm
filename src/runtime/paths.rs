//! Runtime state paths and directory safety checks.

use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

use nix::unistd::{Uid, User};

use crate::constants::{
    DAEMON_STDERR_FILE_NAME, DAEMON_STDOUT_FILE_NAME, DB_FILE_NAME, DEFAULT_USER_STATE_DIR,
    JOB_LOG_DIR_NAME, STATE_DIR_NAME, SYSTEM_STATE_DIR,
};

/// Paths used by a daemon instance.
pub(crate) struct RuntimePaths {
    /// State directory.
    pub(crate) root: PathBuf,
    /// SQLite database.
    pub(crate) db: PathBuf,
    /// Job log directory.
    pub(crate) job_logs: PathBuf,
    /// Daemon standard output.
    pub(crate) stdout: PathBuf,
    /// Daemon standard error.
    pub(crate) stderr: PathBuf,
}

impl RuntimePaths {
    /// Construct all paths from a state directory.
    ///
    /// # Arguments
    ///
    /// * `root` - State directory used as the base for database and log paths.
    ///
    /// # Returns
    ///
    /// Derived paths without filesystem access or validation.
    pub(crate) fn from_root(root: PathBuf) -> Self {
        Self {
            db: root.join(DB_FILE_NAME),
            job_logs: root.join(JOB_LOG_DIR_NAME),
            stdout: root.join(DAEMON_STDOUT_FILE_NAME),
            stderr: root.join(DAEMON_STDERR_FILE_NAME),
            root,
        }
    }

    /// Select the state directory for user or system mode.
    ///
    /// # Arguments
    ///
    /// * `user` - Whether to select personal state.
    ///
    /// # Returns
    ///
    /// Personal paths under nonempty `XDG_STATE_HOME`, falling back to the effective
    /// account's `.local/state`, or fixed system paths. No directories are created.
    pub(crate) fn select(user: bool) -> Result<Self, String> {
        let root = if !user {
            PathBuf::from(SYSTEM_STATE_DIR)
        } else if let Some(value) = std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err("XDG_STATE_HOME must be absolute".into());
            }
            path.join(STATE_DIR_NAME)
        } else {
            let account = User::from_uid(Uid::effective())
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "Current user not found".to_string())?;
            if !account.dir.is_absolute() {
                return Err("User home must be absolute".into());
            }
            account
                .dir
                .join(DEFAULT_USER_STATE_DIR)
                .join(STATE_DIR_NAME)
        };

        Ok(Self::from_root(root))
    }

    /// Create and secure state directories and existing state files.
    ///
    /// Sets the process umask to `0077`; existing database and daemon log files
    /// must be regular, singly linked files owned by the effective user.
    ///
    /// # Arguments
    ///
    /// * `user` - Use `0700` directories when true, otherwise `0711`.
    ///
    /// # Returns
    ///
    /// Success after securing directories and setting existing state files to `0600`.
    pub(crate) fn secure(&self, user: bool) -> Result<(), String> {
        secure_dir(&self.root, if user { 0o700 } else { 0o711 })?;
        secure_dir(&self.job_logs, if user { 0o700 } else { 0o711 })?;
        // SQLite and daemon output must remain private under a traversable system directory.
        unsafe {
            nix::libc::umask(0o077);
        }
        let sqlite = self.sqlite_files();
        for path in sqlite
            .iter()
            .map(PathBuf::as_path)
            .chain([self.stdout.as_path(), self.stderr.as_path()])
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

    /// Derive the database file and its SQLite WAL/shared-memory companions.
    ///
    /// # Returns
    ///
    /// Database, WAL, and shared-memory paths derived from the database path.
    pub(crate) fn sqlite_files(&self) -> [PathBuf; 3] {
        let sidecar = |suffix: &str| {
            let mut path = self.db.as_os_str().to_os_string();
            path.push(suffix);
            PathBuf::from(path)
        };
        [self.db.clone(), sidecar("-wal"), sidecar("-shm")]
    }
}

/// Create a directory and verify its ownership and type.
///
/// # Arguments
///
/// * `path` - Directory to create or inspect; its final component must not be a symlink.
/// * `mode` - Unix permissions to apply after checking effective-user ownership.
///
/// # Returns
///
/// Success after the directory is checked and its permissions are set.
fn secure_dir(path: &Path, mode: u32) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("Failed to create {}: {}", path.display(), error))?;
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.uid() != Uid::effective().as_raw() {
        return Err(format!("Unsafe state directory: {}", path.display()));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("Failed to secure {}: {}", path.display(), error))
}

#[cfg(test)]
mod tests {
    use std::{os::unix::fs::symlink, time::SystemTime};

    use super::*;

    #[test]
    fn constructs_all_paths_from_root() {
        let root = PathBuf::from("/example/cgm");
        let paths = RuntimePaths::from_root(root.clone());
        assert_eq!(paths.root, root);
        assert_eq!(paths.db, root.join("cgm.db"));
        assert_eq!(
            paths.sqlite_files(),
            [
                root.join("cgm.db"),
                root.join("cgm.db-wal"),
                root.join("cgm.db-shm")
            ]
        );
        assert_eq!(paths.job_logs, root.join("logs"));
        assert_eq!(paths.stdout, root.join("cgm.out"));
        assert_eq!(paths.stderr, root.join("cgm.err"));
    }

    #[test]
    fn state_directory_is_private_and_rejects_symlinks() {
        let root = std::env::temp_dir().join(format!(
            "cgm-state-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let state = root.join("state");
        secure_dir(&state, 0o700).unwrap();
        assert_eq!(
            fs::metadata(&state).unwrap().permissions().mode() & 0o777,
            0o700
        );
        secure_dir(&state, 0o711).unwrap();
        assert_eq!(
            fs::metadata(&state).unwrap().permissions().mode() & 0o777,
            0o711
        );
        let link = root.join("link");
        symlink(&state, &link).unwrap();
        assert!(secure_dir(&link, 0o700).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
