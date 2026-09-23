//! Tauri-independent, allowlisted telemetry. Provider credentials are never opened here.
pub mod analytics;
pub mod alerts;
pub mod cli;
pub mod config;
pub mod history;
pub mod observatory;
pub mod commands;
mod storage;
mod ingest;
mod model;
pub use history::History;
pub use ingest::{parse_legacy, parse_statusline};
pub use model::*;

mod everywhere;
mod integrations;
mod local_api;
mod maintenance;
mod private_fs;
mod terminal;
