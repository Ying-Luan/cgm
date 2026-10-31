//! Constants definitions
//!
//! Fixed socket name, state layout, configuration paths, and installation guide.

/// SQLite database filename relative to the state directory.
pub(crate) const DB_FILE_NAME: &str = "cgm.db";
/// Daemon standard output filename relative to the state directory.
pub(crate) const DAEMON_STDOUT_FILE_NAME: &str = "cgm.out";
/// Daemon standard error filename relative to the state directory.
pub(crate) const DAEMON_STDERR_FILE_NAME: &str = "cgm.err";

/// Task log directory relative to the state directory.
pub(crate) const JOB_LOG_DIR_NAME: &str = "logs";
/// Default task log filename prefix, followed by the job ID.
pub(crate) const JOB_LOG_FILE_PREFIX: &str = "job-";
/// Default task log filename suffix.
pub(crate) const JOB_LOG_FILE_SUFFIX: &str = ".log";

/// Application directory beneath the selected user state location.
pub(crate) const STATE_DIR_NAME: &str = "cgm";
/// Persistent data root for the system daemon.
pub(crate) const SYSTEM_STATE_DIR: &str = "/var/lib/cgm";
/// Default user state directory relative to the account home.
pub(crate) const DEFAULT_USER_STATE_DIR: &str = ".local/state";

/// System-wide configuration path.
pub(crate) const SYSTEM_CONFIG_PATH: &str = "/etc/cgm/config.toml";
/// Default user configuration directory relative to the account home.
pub(crate) const DEFAULT_USER_CONFIG_DIR: &str = ".config";
/// User configuration path relative to the selected configuration directory.
pub(crate) const USER_CONFIG_PATH: &str = "cgm/config.toml";

/// Stable machine-wide Linux abstract socket name.
pub(crate) const SOCKET_NAME: &[u8] = b"cgm.daemon";

/// Installation instructions linked from startup errors.
pub(crate) const INSTALL_GUIDE_URL: &str = "https://github.com/Ying-Luan/cgm#installation";
