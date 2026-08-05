//! Configuration module.

mod load;
mod manage;
mod path;
mod validate;

pub(crate) use load::{load_effective, load_global_effective};
pub(crate) use manage::{get, init, set, show, unset, validate_current};
pub(crate) use path::config_path;
pub(crate) use validate::validate_effective;
