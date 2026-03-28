//! Database CRUD operations
//!
//! Provides DataBase struct wrapping SQLite connection, supports job CRUD operations.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, params};

use crate::{
    db::schema::initialize_db,
    types::{Job, JobStatus},
};

/// Get current Unix timestamp
///
/// # Returns
///
/// Current Unix timestamp in seconds
fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Check if database schema is compatible with current code
///
/// Uses PRAGMA table_info to get table structure, checks if columns exactly match required set.
///
/// # Arguments
///
/// * `conn` - SQLite connection
///
/// # Returns
///
/// True if compatible, false if incompatible (e.g. missing columns, extra columns, or column name changes)
pub fn check_db_compatible(conn: &Connection) -> bool {
    // Get jobs table column names
    let mut stmt = conn.prepare("PRAGMA table_info(jobs)").unwrap();
    let columns: HashSet<String> = stmt
        .query_map([], |row| row.get(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    // Required column names
    let required: HashSet<String> = [
        "id",
        "status",
        "username",
        "command",
        "gpus",
        "envs",
        "log_path",
        "cwd",
        "start_time",
        "end_time",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    columns == required
}

/// Database struct
pub struct DataBase {
    /// SQLite connection
    conn: Connection,
}

/// Database connection pool type alias
pub type DbPool = Arc<Mutex<DataBase>>;

impl DataBase {
    /// Open (or create) database, run migrations, return thread-safe connection handle
    ///
    /// # Arguments
    ///
    /// * `db_path` - Database file path
    ///
    /// # Returns
    ///
    /// Thread-safe database connection pool
    pub fn open(db_path: &Path) -> rusqlite::Result<DbPool> {
        let conn = Connection::open(db_path)?;

        // Enable WAL mode: allows concurrent reads, reduces write lock contention
        conn.pragma_update(None, "journal_mode", "WAL")?;

        initialize_db(&conn)?;

        Ok(Arc::new(Mutex::new(DataBase { conn })))
    }

    /// Insert a job
    ///
    /// # Arguments
    ///
    /// * `job` - Job struct to insert
    ///
    /// # Returns
    ///
    /// Insert result, Ok(()) on success, error message on failure
    pub fn insert_job(&self, job: &Job) -> rusqlite::Result<()> {
        let command_json = serde_json::to_string(&job.command).unwrap();
        let envs_json = serde_json::to_string(&job.envs).unwrap();
        self.conn.execute(
            "INSERT INTO jobs (id, status, username, command, gpus, envs, log_path, cwd, start_time, end_time)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                job.id as i64,
                job.status.as_str(),
                job.username,
                command_json,
                job.gpus as i32,
                envs_json,
                job.log_path,
                job.cwd,
                job.start_time,
                job.end_time,
            ],
        )?;

        Ok(())
    }

    /// Delete jobs by IDs
    ///
    /// # Arguments
    ///
    /// * `ids` - List of job IDs to delete
    ///
    /// # Returns
    ///
    /// Number of deleted jobs on success, error message on failure
    pub fn delete_jobs(&self, ids: &[usize]) -> Result<usize, String> {
        if ids.is_empty() {
            return Ok(0);
        }
        let placeholders: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM jobs WHERE id IN ({})", placeholders);
        let params: Vec<i32> = ids.iter().map(|&id| id as i32).collect();

        self.conn
            .execute(&sql, rusqlite::params_from_iter(&params))
            .map_err(|e| e.to_string())
    }

    /// Get job IDs by statuses
    ///
    /// # Arguments
    ///
    /// * `statuses` - List of job statuses to filter
    ///
    /// # Returns
    ///
    /// Job list, `Ok(Vec<Job>)` on success, error message on failure
    pub fn get_jobs_by_statuses(&self, statuses: &[JobStatus]) -> Result<Vec<Job>, String> {
        let placeholders: String = statuses.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let condition = format!("WHERE status IN ({}) ORDER BY id ASC", placeholders);
        let status_strs: Vec<&str> = statuses.iter().map(|s| s.as_str()).collect();

        self.query_jobs(&condition, rusqlite::params_from_iter(&status_strs))
            .map_err(|e| e.to_string())
    }

    /// Update job status
    ///
    /// Automatically sets start_time when status becomes Running,
    /// and end_time when status becomes Completed, Failed, or Cancelled.
    ///
    /// # Arguments
    ///
    /// * `job_id` - Job ID
    /// * `status` - New job status
    ///
    /// # Returns
    ///
    /// Update result, Ok(()) on success, error message on failure
    pub fn update_status(&self, job_id: usize, status: JobStatus) -> rusqlite::Result<()> {
        let now = now_timestamp();

        match status {
            JobStatus::Running => {
                self.conn.execute(
                    "UPDATE jobs SET status = ?1, start_time = ?2 WHERE id = ?3",
                    params![status.as_str(), now, job_id as i32],
                )?;
            }
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled => {
                self.conn.execute(
                    "UPDATE jobs SET status = ?1, end_time = ?2 WHERE id = ?3",
                    params![status.as_str(), now, job_id as i32],
                )?;
            }
            _ => {
                self.conn.execute(
                    "UPDATE jobs SET status = ?1 WHERE id = ?2",
                    params![status.as_str(), job_id as i32],
                )?;
            }
        }

        Ok(())
    }

    /// Get all jobs, ordered by ID ascending (FIFO)
    ///
    /// # Arguments
    ///
    /// * `limit` - Limit number of jobs returned, None for all
    ///
    /// # Returns
    ///
    /// Job list, `Ok(Vec<Job>)` on success, error message on failure
    pub fn get_all_jobs(&self, limit: Option<usize>) -> rusqlite::Result<Vec<Job>> {
        match limit {
            Some(n) => {
                let condition = "ORDER BY id DESC LIMIT ?1";
                let mut jobs = self.query_jobs(condition, params![n as i32])?;
                jobs.reverse();
                Ok(jobs)
            }
            None => {
                let condition = "ORDER BY id ASC";
                self.query_jobs(condition, [])
            }
        }
    }

    /// Query single job by ID
    ///
    /// # Arguments
    ///
    /// * `job_id` - Job ID
    ///
    /// # Returns
    ///
    /// Query result, `Ok(Some(Job))` or `Ok(None)` on success, error message on failure
    pub fn get_job_by_id(&self, job_id: usize) -> rusqlite::Result<Option<Job>> {
        let condition = "WHERE id = ?1";
        let mut rows = self.query_jobs(condition, params![job_id as i32])?;

        Ok(rows.pop())
    }

    /// Get job count in queue
    ///
    /// # Returns
    ///
    /// Job count, Ok(count) on success, error message on failure
    pub fn get_job_count(&self) -> rusqlite::Result<i32> {
        self.conn
            .query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))
    }

    /// Get max job ID
    ///
    /// # Returns
    ///
    /// Max ID, returns 0 if no jobs
    pub fn get_max_id(&self) -> rusqlite::Result<usize> {
        self.conn
            .query_row("SELECT MAX(id) FROM jobs", [], |row| {
                row.get::<_, Option<i64>>(0)
            })
            .map(|id: Option<i64>| id.unwrap_or(0) as usize)
    }

    /// Get jobs by status
    ///
    /// # Arguments
    ///
    /// * `status` - Job status
    ///
    /// # Returns
    ///
    /// Job list, `Ok(Vec<Job>)` on success, error message on failure
    pub fn get_jobs_by_status(&self, status: JobStatus) -> rusqlite::Result<Vec<Job>> {
        let condition = "WHERE status = ?1 ORDER BY id ASC";

        self.query_jobs(condition, params![status.as_str()])
    }

    /// Get jobs by username
    ///
    /// # Arguments
    ///
    /// * `username` - Username
    /// * `limit` - Limit number of jobs returned, None for all
    ///
    /// # Returns
    ///
    /// Job list
    pub fn get_jobs_by_username(
        &self,
        username: &str,
        limit: Option<usize>,
    ) -> rusqlite::Result<Vec<Job>> {
        match limit {
            Some(n) => {
                let condition = "WHERE username = ?1 ORDER BY id DESC LIMIT ?2";
                let mut jobs = self.query_jobs(condition, params![username, n as i32])?;
                jobs.reverse();
                Ok(jobs)
            }
            None => {
                let condition = "WHERE username = ?1 ORDER BY id ASC";
                self.query_jobs(condition, params![username])
            }
        }
    }

    /// Internal generic query function, executes query and maps results to Job struct list
    ///
    /// # Arguments
    ///
    /// * `condition` - SQL condition string (e.g. "WHERE status = ?1 ORDER BY id ASC")
    /// * `params` - Query parameters
    ///
    /// # Returns
    ///
    /// Query result, `Ok(Vec<Job>)` on success, error message on failure
    fn query_jobs<P: rusqlite::Params>(
        &self,
        condition: &str,
        params: P,
    ) -> rusqlite::Result<Vec<Job>> {
        let sql = format!(
            "SELECT id, status, username, command, gpus, envs, log_path, cwd, start_time, end_time FROM jobs {}",
            condition
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params, |row| {
            // Extract fields from query result, construct Job struct
            let id_int: i64 = row.get(0)?;
            let status_str: String = row.get(1)?;
            let status = JobStatus::from_str(&status_str);
            let command_json: String = row.get(3)?;
            let command: Vec<String> = serde_json::from_str(&command_json).unwrap_or_default();
            let gpus_int: i64 = row.get(4)?;
            let envs_json: String = row.get(5)?;
            let envs: HashMap<String, String> =
                serde_json::from_str(&envs_json).unwrap_or_default();
            let log_path: String = row.get(6)?;
            let cwd: String = row.get(7)?;
            let start_time: Option<i64> = row.get(8)?;
            let end_time: Option<i64> = row.get(9)?;

            // Construct and return Job struct
            Ok(Job {
                id: id_int as usize,
                status,
                username: row.get(2)?,
                command,
                gpus: gpus_int as usize,
                envs,
                log_path,
                cwd,
                start_time,
                end_time,
            })
        })?;

        rows.collect()
    }
}
