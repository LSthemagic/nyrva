//! Tauri-independent, allowlisted telemetry. No network or credential access.
mod ingest;
mod model;
pub mod analytics;
pub mod cli;
pub mod history;
pub use ingest::{parse_legacy, parse_statusline};
pub use model::*;
pub use history::History;
