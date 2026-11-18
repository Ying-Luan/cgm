//! Database module
//!
//! Provides DataBase struct wrapping SQLite connection, schema initialization and CRUD operations.

mod migrate;
mod operations;
mod schema;

pub(crate) use operations::{DataBase, DbPool};
