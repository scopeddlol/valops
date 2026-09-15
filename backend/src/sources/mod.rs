//! External match sources.
//!
//! A source's only job is to produce [`crate::import::IncomingMatch`] values;
//! everything after that — deduplication, player resolution, the dry-run
//! preview — is the shared import core's problem.

pub mod henrik;
