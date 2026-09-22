//! Tauri-independent, allowlisted telemetry. No network or credential access.
pub mod analytics;
pub mod cli;
pub mod history;
mod ingest;
mod model;
pub use history::History;
pub use ingest::{parse_legacy, parse_statusline};
pub use model::*;
