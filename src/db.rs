//! Database module
//!
//! Provides DataBase struct wrapping SQLite connection, schema initialization and CRUD operations.

mod operations;
mod schema;

pub use operations::{DataBase, DbPool, check_db_compatible};
