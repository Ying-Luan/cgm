//! Database CRUD operations
//!
//! Provides DataBase struct wrapping SQLite connection, supports job CRUD operations.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    db::schema::initialize_db,
    types::{GpuSelection, Job, JobStatus},
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
pub(crate) fn check_db_compatible(conn: &Connection) -> bool {
    check_table_columns(
        conn,
        &[
            "id",
            "status",
            "username",
            "command",
            "allowed_gpus",
            "required_gpus",
            "envs",
            "log_path",
            "cwd",
            "remaining_dependencies",
            "start_time",
            "end_time",
        ],
        "jobs",
    ) && check_table_columns(conn, &["job_id", "depends_on"], "job_dependencies")
}

/// Check a database table's columns against the required set.
///
/// # Arguments
///
/// * `conn` - SQLite connection
/// * `required` - Required column names
/// * `table` - Table name from the fixed schema
///
/// # Returns
///
/// Whether the table columns exactly match the required set or not.
fn check_table_columns(conn: &Connection, required: &[&str], table: &str) -> bool {
    let Ok(mut stmt) = conn.prepare(&format!("PRAGMA table_info({})", table)) else {
        return false;
    };
    let Ok(columns) = stmt.query_map([], |row| row.get(1)) else {
        return false;
    };
    let columns: HashSet<String> = columns.filter_map(Result::ok).collect();
    let required: HashSet<String> = required.iter().map(|column| column.to_string()).collect();

    columns == required
}

/// Database struct
pub(crate) struct DataBase {
    /// SQLite connection
    conn: Connection,
}

/// Database connection pool type alias
pub(crate) type DbPool = Arc<Mutex<DataBase>>;

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
    pub(crate) fn open(db_path: &Path) -> rusqlite::Result<DbPool> {
        let conn = Connection::open(db_path)?;

        // Enable WAL mode: allows concurrent reads, reduces write lock contention
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // Enable foreign key constraints: ensures referential integrity for job dependencies
        conn.pragma_update(None, "foreign_keys", true)?;

        initialize_db(&conn)?;

        Ok(Arc::new(Mutex::new(DataBase { conn })))
    }

    /// Insert a job and its prerequisite relationships
    ///
    /// # Arguments
    ///
    /// * `dependencies` - Prerequisite job IDs
    /// * `job` - Job struct to insert and initialize
    ///
    /// # Returns
    ///
    /// Insert result, Ok(()) on success, error message on failure
    pub(crate) fn insert_job_with_dependencies(
        &self,
        dependencies: &[usize],
        job: &mut Job,
    ) -> Result<(), String> {
        let transaction = self
            .conn
            .unchecked_transaction()
            .map_err(|error| error.to_string())?;
        let mut failed_dependency = false;
        let mut remaining_dependencies = 0;

        for &dependency in dependencies {
            if dependency == job.id {
                return Err(format!("Job {} cannot depend on itself", job.id));
            }

            let status = transaction
                .query_row(
                    "SELECT status FROM jobs WHERE id = ?1",
                    params![dependency as i64],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("Dependency job {} not found", dependency))?;
            match JobStatus::from_str(&status) {
                JobStatus::Completed => {}
                JobStatus::Cancelled | JobStatus::Failed => failed_dependency = true,
                _ => remaining_dependencies += 1,
            }
        }

        job.remaining_dependencies = remaining_dependencies;
        job.status = if failed_dependency {
            job.end_time = Some(now_timestamp());
            JobStatus::Failed
        } else if remaining_dependencies == 0 {
            JobStatus::Pending
        } else {
            JobStatus::Blocked
        };

        let command_json = serde_json::to_string(&job.command).map_err(|e| e.to_string())?;
        let allowed_gpus_json =
            serde_json::to_string(&job.allowed_gpus).map_err(|e| e.to_string())?;
        let envs_json = serde_json::to_string(&job.envs).map_err(|e| e.to_string())?;
        transaction
            .execute(
                "INSERT INTO jobs (id, status, username, command, allowed_gpus, required_gpus, envs, log_path, cwd, remaining_dependencies, start_time, end_time)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                job.id as i64,
                job.status.as_str(),
                job.username,
                command_json,
                allowed_gpus_json,
                job.required_gpus as i32,
                envs_json,
                job.log_path,
                job.cwd,
                job.remaining_dependencies as i64,
                job.start_time,
                job.end_time,
            ],
            )
            .map_err(|e| e.to_string())?;
        for &dependency in dependencies {
            transaction
                .execute(
                    "INSERT INTO job_dependencies (job_id, depends_on) VALUES (?1, ?2)",
                    params![job.id as i64, dependency as i64],
                )
                .map_err(|e| e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())?;

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
    pub(crate) fn delete_jobs(&self, ids: &[usize]) -> Result<usize, String> {
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
    pub(crate) fn get_jobs_by_statuses(&self, statuses: &[JobStatus]) -> Result<Vec<Job>, String> {
        let placeholders: String = statuses.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let condition = format!("WHERE status IN ({}) ORDER BY id ASC", placeholders);
        let status_strs: Vec<&str> = statuses.iter().map(|s| s.as_str()).collect();

        self.query_jobs(&condition, rusqlite::params_from_iter(&status_strs))
            .map_err(|e| e.to_string())
    }

    /// Update job status
    ///
    /// Automatically maintains timestamps and prerequisite state transitions.
    ///
    /// # Arguments
    ///
    /// * `job_id` - Job ID
    /// * `status` - New job status
    ///
    /// # Returns
    ///
    /// Update result, Ok(()) on success, error message on failure
    pub(crate) fn update_status(&self, job_id: usize, status: JobStatus) -> rusqlite::Result<()> {
        let now = now_timestamp();

        match status {
            JobStatus::Running => {
                self.conn.execute(
                    "UPDATE jobs SET status = ?1, start_time = ?2 WHERE id = ?3",
                    params![status.as_str(), now, job_id as i32],
                )?;
            }
            JobStatus::Completed => {
                let transaction = self.conn.unchecked_transaction()?;
                let updated = transaction.execute(
                    "UPDATE jobs SET status = 'completed', end_time = ?1 WHERE id = ?2",
                    params![now, job_id as i64],
                )?;

                if updated > 0 {
                    transaction.execute(
                        "UPDATE jobs
                         SET remaining_dependencies = remaining_dependencies - 1,
                             status = CASE
                                 WHEN remaining_dependencies = 1 THEN 'pending'
                                 ELSE status
                             END
                         WHERE id IN (
                             SELECT job_id
                             FROM job_dependencies
                             WHERE depends_on = ?1
                         )",
                        params![job_id as i64],
                    )?;
                }

                transaction.commit()?;
            }
            JobStatus::Cancelled | JobStatus::Failed => {
                let transaction = self.conn.unchecked_transaction()?;
                let updated = transaction.execute(
                    "UPDATE jobs SET status = ?1, end_time = ?2 WHERE id = ?3",
                    params![status.as_str(), now, job_id as i64],
                )?;

                if updated > 0 {
                    transaction.execute(
                        "WITH RECURSIVE descendants(id) AS (
                             SELECT job_id
                             FROM job_dependencies
                             WHERE depends_on = ?1
                             UNION
                             SELECT dependency.job_id
                             FROM job_dependencies dependency
                             JOIN descendants
                               ON dependency.depends_on = descendants.id
                         )
                         UPDATE jobs
                         SET status = 'failed',
                             end_time = ?2
                         WHERE id IN (SELECT id FROM descendants)",
                        params![job_id as i64, now],
                    )?;
                }

                transaction.commit()?;
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
    pub(crate) fn get_all_jobs(&self, limit: Option<usize>) -> rusqlite::Result<Vec<Job>> {
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
    pub(crate) fn get_job_by_id(&self, job_id: usize) -> rusqlite::Result<Option<Job>> {
        let condition = "WHERE id = ?1";
        let mut rows = self.query_jobs(condition, params![job_id as i32])?;

        Ok(rows.pop())
    }

    /// Get job count in queue
    ///
    /// # Returns
    ///
    /// Job count, Ok(count) on success, error message on failure
    pub(crate) fn get_job_count(&self) -> rusqlite::Result<i32> {
        self.conn
            .query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))
    }

    /// Get max job ID
    ///
    /// # Returns
    ///
    /// Max ID, returns 0 if no jobs
    pub(crate) fn get_max_id(&self) -> rusqlite::Result<usize> {
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
    pub(crate) fn get_jobs_by_status(&self, status: JobStatus) -> rusqlite::Result<Vec<Job>> {
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
    pub(crate) fn get_jobs_by_username(
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

    /// Get prerequisite job IDs grouped by dependent job ID.
    ///
    /// # Arguments
    ///
    /// * `job_ids` - Dependent job IDs
    ///
    /// # Returns
    ///
    /// Prerequisite IDs grouped by job ID.
    pub(crate) fn get_dependencies_by_job_ids(
        &self,
        job_ids: &[usize],
    ) -> rusqlite::Result<HashMap<usize, Vec<usize>>> {
        if job_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders = job_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT job_id, depends_on FROM job_dependencies WHERE job_id IN ({}) ORDER BY job_id ASC, depends_on ASC",
            placeholders
        );
        let params = job_ids.iter().map(|&id| id as i64).collect::<Vec<_>>();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(&params), |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                row.get::<_, i64>(1)? as usize,
            ))
        })?;
        let mut dependencies = HashMap::new();
        for row in rows {
            let (job_id, depends_on) = row?;
            dependencies
                .entry(job_id)
                .or_insert_with(Vec::new)
                .push(depends_on);
        }

        Ok(dependencies)
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
            "SELECT id, status, username, command, allowed_gpus, required_gpus, envs, log_path, cwd, remaining_dependencies, start_time, end_time FROM jobs {}",
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
            let allowed_gpus_json: String = row.get(4)?;
            let allowed_gpus: GpuSelection =
                serde_json::from_str(&allowed_gpus_json).unwrap_or(GpuSelection::All);
            let required_gpus_int: i64 = row.get(5)?;
            let envs_json: String = row.get(6)?;
            let envs: HashMap<String, String> =
                serde_json::from_str(&envs_json).unwrap_or_default();
            let log_path: String = row.get(7)?;
            let cwd: String = row.get(8)?;
            let remaining_dependencies: i64 = row.get(9)?;
            let start_time: Option<i64> = row.get(10)?;
            let end_time: Option<i64> = row.get(11)?;

            // Construct and return Job struct
            Ok(Job {
                id: id_int as usize,
                status,
                username: row.get(2)?,
                command,
                allowed_gpus,
                required_gpus: required_gpus_int as usize,
                envs,
                log_path,
                cwd,
                remaining_dependencies: remaining_dependencies as usize,
                start_time,
                end_time,
            })
        })?;

        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rusqlite::Connection;

    use super::DataBase;
    use crate::{
        db::schema::initialize_db,
        types::{GpuSelection, Job, JobStatus},
    };

    /// Create an in-memory database with the production schema and constraints.
    fn database() -> DataBase {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        initialize_db(&conn).unwrap();
        DataBase { conn }
    }

    /// Create a pending job for dependency tests.
    fn job(id: usize) -> Job {
        Job {
            id,
            status: JobStatus::Pending,
            username: "user".to_string(),
            command: vec!["true".to_string()],
            allowed_gpus: GpuSelection::All,
            required_gpus: 1,
            envs: HashMap::new(),
            log_path: format!("/tmp/job-{}.log", id),
            cwd: "/tmp".to_string(),
            remaining_dependencies: 0,
            start_time: None,
            end_time: None,
        }
    }

    /// Insert a test job and return its persisted representation.
    fn insert(db: &DataBase, dependencies: &[usize], id: usize) -> Job {
        let mut job = job(id);
        db.insert_job_with_dependencies(dependencies, &mut job)
            .unwrap();
        db.get_job_by_id(id).unwrap().unwrap()
    }

    #[test]
    fn inserts_blocked_job_and_loads_dependencies() {
        let db = database();
        insert(&db, &[], 1);
        let dependent = insert(&db, &[1], 2);

        assert_eq!(dependent.status, JobStatus::Blocked);
        assert_eq!(dependent.remaining_dependencies, 1);
        assert_eq!(
            db.get_dependencies_by_job_ids(&[2]).unwrap().get(&2),
            Some(&vec![1])
        );
    }

    #[test]
    fn rejects_missing_and_duplicate_dependencies() {
        let db = database();
        insert(&db, &[], 1);

        assert!(
            db.insert_job_with_dependencies(&[99], &mut job(2))
                .unwrap_err()
                .contains("not found")
        );
        assert!(
            db.insert_job_with_dependencies(&[2], &mut job(2))
                .unwrap_err()
                .contains("itself")
        );
        assert!(
            db.insert_job_with_dependencies(&[1, 1], &mut job(2))
                .is_err()
        );
    }

    #[test]
    fn completing_last_dependency_releases_blocked_job() {
        let db = database();
        insert(&db, &[], 1);
        insert(&db, &[], 2);
        insert(&db, &[1, 2], 3);
        db.update_status(1, JobStatus::Running).unwrap();

        db.update_status(1, JobStatus::Completed).unwrap();

        let prerequisite = db.get_job_by_id(1).unwrap().unwrap();
        let dependent = db.get_job_by_id(3).unwrap().unwrap();
        assert_eq!(prerequisite.status, JobStatus::Completed);
        assert_eq!(dependent.status, JobStatus::Blocked);
        assert_eq!(dependent.remaining_dependencies, 1);

        db.update_status(2, JobStatus::Running).unwrap();
        db.update_status(2, JobStatus::Completed).unwrap();
        let dependent = db.get_job_by_id(3).unwrap().unwrap();
        assert_eq!(dependent.status, JobStatus::Pending);
        assert_eq!(dependent.remaining_dependencies, 0);
    }

    #[test]
    fn completed_dependency_does_not_block_new_job() {
        let db = database();
        insert(&db, &[], 1);
        db.update_status(1, JobStatus::Running).unwrap();
        db.update_status(1, JobStatus::Completed).unwrap();

        let dependent = insert(&db, &[1], 2);
        assert_eq!(dependent.status, JobStatus::Pending);
        assert_eq!(dependent.remaining_dependencies, 0);
    }

    #[test]
    fn persists_allowed_gpus() {
        let db = database();
        let mut job = job(1);
        job.allowed_gpus = GpuSelection::Allowed(vec![0, 2]);

        db.insert_job_with_dependencies(&[], &mut job).unwrap();

        assert_eq!(
            db.get_job_by_id(1).unwrap().unwrap().allowed_gpus,
            GpuSelection::Allowed(vec![0, 2])
        );
    }

    #[test]
    fn failed_dependency_fails_blocked_descendants() {
        let db = database();
        insert(&db, &[], 1);
        insert(&db, &[1], 2);
        insert(&db, &[2], 3);

        db.update_status(1, JobStatus::Failed).unwrap();

        assert_eq!(
            db.get_job_by_id(1).unwrap().unwrap().status,
            JobStatus::Failed
        );
        assert_eq!(
            db.get_job_by_id(2).unwrap().unwrap().status,
            JobStatus::Failed
        );
        assert_eq!(
            db.get_job_by_id(3).unwrap().unwrap().status,
            JobStatus::Failed
        );
    }

    #[test]
    fn cancelled_dependency_fails_blocked_descendants() {
        let db = database();
        insert(&db, &[], 1);
        insert(&db, &[1], 2);
        insert(&db, &[2], 3);

        db.update_status(1, JobStatus::Cancelled).unwrap();

        assert_eq!(
            db.get_job_by_id(1).unwrap().unwrap().status,
            JobStatus::Cancelled
        );
        assert_eq!(
            db.get_job_by_id(2).unwrap().unwrap().status,
            JobStatus::Failed
        );
        assert_eq!(
            db.get_job_by_id(3).unwrap().unwrap().status,
            JobStatus::Failed
        );
    }

    #[test]
    fn deleting_prerequisite_is_restricted() {
        let db = database();
        insert(&db, &[], 1);
        insert(&db, &[1], 2);

        assert!(db.delete_jobs(&[1]).is_err());
        assert_eq!(db.delete_jobs(&[2]).unwrap(), 1);
        assert_eq!(db.delete_jobs(&[1]).unwrap(), 1);
    }

    #[test]
    fn deleting_related_jobs_together_is_allowed() {
        let db = database();
        insert(&db, &[], 1);
        insert(&db, &[1], 2);

        assert_eq!(db.delete_jobs(&[1, 2]).unwrap(), 2);
    }
}
