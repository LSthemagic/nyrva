//! Tauri-independent, allowlisted telemetry. No network or credential access.
mod ingest;
mod model;
pub use ingest::{parse_legacy, parse_statusline};
pub use model::*;
