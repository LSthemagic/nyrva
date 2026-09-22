//! Tauri-independent telemetry contracts. Initial conservative collector baseline.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider { Claude, Codex, Cursor, Antigravity }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataStatus { Live, Stale, Unavailable, Unsupported, NeedsAuth, Backoff, Error }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bucket {
    pub id: String,
    pub label: String,
    pub remaining_fraction: Option<f64>,
    pub resets_at_ms: Option<u64>,
    pub count: Option<u64>,
    pub estimated: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextUsage { pub used_percentage: Option<f64>, pub size: Option<u64> }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub provider: Provider,
    pub account_id: String,
    pub source: String,
    pub observed_at_ms: u64,
    pub status: DataStatus,
    pub buckets: Vec<Bucket>,
    pub context: Option<ContextUsage>,
    pub model: Option<String>,
}
impl Snapshot {
    pub fn effective_status(&self, now_ms: u64) -> DataStatus {
        if self.status == DataStatus::Live && now_ms.saturating_sub(self.observed_at_ms) > 600_000 { DataStatus::Stale } else { self.status }
    }
}
/// Conservative baseline: until parsing is implemented, missing information stays unavailable.
pub fn parse_statusline(provider: Provider, account: &str, _input: &str, now_ms: u64) -> Result<Snapshot, String> {
    Ok(Snapshot { schema_version: 1, provider, account_id: account.into(), source: "statusline".into(), observed_at_ms: now_ms, status: DataStatus::Unavailable, buckets: vec![], context: None, model: None })
}
