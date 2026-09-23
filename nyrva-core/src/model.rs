use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
pub const MAX_BUCKETS: usize = 128;
pub const FRESH_FOR_MS: u64 = 600_000;
pub const MAX_TIMESTAMP_MS: u64 = 253_402_300_799_999;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider { Claude, Codex, Cursor, Antigravity }
impl Provider {
    pub fn as_str(self) -> &'static str {
        match self { Self::Claude => "claude", Self::Codex => "codex", Self::Cursor => "cursor", Self::Antigravity => "antigravity" }
    }
}
impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
}
impl FromStr for Provider {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s { "claude" => Ok(Self::Claude), "codex" => Ok(Self::Codex), "cursor" => Ok(Self::Cursor), "antigravity" | "gemini" => Ok(Self::Antigravity), _ => Err("unknown provider".into()) }
    }
}
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
    #[serde(default)] pub window_seconds: Option<u64>,
}
impl Bucket {
    pub fn reset_in_ms(&self, now_ms: u64) -> Option<u64> { self.resets_at_ms.map(|t| t.saturating_sub(now_ms)) }
    pub fn awaiting_reset_confirmation(&self, now_ms: u64) -> bool { self.resets_at_ms.is_some_and(|t| t <= now_ms) }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextUsage {
    pub used_percentage: Option<f64>,
    pub size: Option<u64>,
    #[serde(default)] pub input_tokens: Option<u64>,
    #[serde(default)] pub output_tokens: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionMetadata {
    pub id: Option<String>,
    pub parent_id: Option<String>,
    pub project: Option<String>,
    pub branch: Option<String>,
    pub state: Option<String>,
}
/// Cumulative session measurements reported by the source, not a provider bill.
/// Never sum repeated snapshots of these totals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SessionMetrics {
    pub estimated_cost_usd: Option<f64>,
    pub duration_ms: Option<u64>,
    pub effort: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub provider: Provider,
    /// User-selected local alias, never a provider email/token.
    pub account_id: String,
    /// Timestamp is source observation, not independent backend verification.
    pub source: String,
    pub observed_at_ms: u64,
    pub status: DataStatus,
    pub buckets: Vec<Bucket>,
    pub context: Option<ContextUsage>,
    pub model: Option<String>,
    #[serde(default)] pub source_version: Option<String>,
    #[serde(default)] pub session: Option<SessionMetadata>,
    #[serde(default)] pub metrics: Option<SessionMetrics>,
    #[serde(default)] pub dropped_fields: u32,
}
impl Snapshot {
    pub fn empty(provider: Provider, account: &str, source: &str, now_ms: u64) -> Self {
        Self { schema_version: SCHEMA_VERSION, provider, account_id: account.into(), source: source.into(), observed_at_ms: now_ms, status: DataStatus::Unavailable, buckets: vec![], context: None, model: None, source_version: None, session: None, metrics: None, dropped_fields: 0 }
    }
    pub fn is_recent(&self, now_ms: u64) -> bool {
        self.observed_at_ms != 0 && self.observed_at_ms <= now_ms.saturating_add(30_000) && now_ms.saturating_sub(self.observed_at_ms) <= FRESH_FOR_MS
    }
    pub fn effective_status(&self, now_ms: u64) -> DataStatus {
        if self.status == DataStatus::Live && !self.is_recent(now_ms) { DataStatus::Stale } else { self.status }
    }
    pub fn validate(&self) -> Result<(), String> {
        validate_account(&self.account_id)?;
        if self.schema_version != SCHEMA_VERSION { return Err("unsupported telemetry schema".into()); }
        if !["statusline", "legacy_adapter"].contains(&self.source.as_str()) { return Err("unsupported telemetry source".into()); }
        if self.observed_at_ms > MAX_TIMESTAMP_MS || self.buckets.len() > MAX_BUCKETS { return Err("telemetry limits exceeded".into()); }
        for value in [&self.model, &self.source_version].into_iter().flatten() {
            if value != &clean_text(value) { return Err("invalid telemetry metadata".into()); }
        }
        if let Some(session) = &self.session {
            for value in [&session.id, &session.parent_id, &session.project, &session.branch, &session.state].into_iter().flatten() {
                if value != &clean_text(value) { return Err("invalid session metadata".into()); }
            }
        }
        if let Some(m) = &self.metrics {
            if m.estimated_cost_usd.is_some_and(|n| !n.is_finite() || !(0.0..=1_000_000_000.0).contains(&n))
                || m.duration_ms.is_some_and(|n| n > MAX_SAFE_INTEGER)
                || m.effort.as_ref().is_some_and(|s| s != &clean_text(s)) {
                return Err("invalid session measurement".into());
            }
        }
        if let Some(c) = &self.context {
            if c.used_percentage.is_some_and(|n| !n.is_finite() || !(0.0..=100.0).contains(&n))
                || c.size.is_some_and(|n| n == 0 || n > MAX_SAFE_INTEGER)
                || c.input_tokens.is_some_and(|n| n > MAX_SAFE_INTEGER)
                || c.output_tokens.is_some_and(|n| n > MAX_SAFE_INTEGER) {
                return Err("invalid context measurement".into());
            }
        }
        let mut ids = std::collections::HashSet::new();
        for b in &self.buckets {
            if b.id.is_empty() || b.id.len() > 512 || b.label.chars().count() > 120
                || b.id.chars().chain(b.label.chars()).any(unsafe_character) || !ids.insert(&b.id) {
                return Err("invalid or duplicate bucket identity".into());
            }
            if b.remaining_fraction.is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v)) { return Err("invalid quota fraction".into()); }
            if b.resets_at_ms.is_some_and(|v| v > MAX_TIMESTAMP_MS) { return Err("invalid reset timestamp".into()); }
            if b.count.is_some() && b.remaining_fraction.is_some() { return Err("count without denominator must not claim a fraction".into()); }
            if b.count.is_some_and(|n| n > MAX_SAFE_INTEGER) || b.window_seconds.is_some_and(|n| n == 0 || n > MAX_TIMESTAMP_MS / 1000) {
                return Err("invalid bucket measurement".into());
            }
        }
        Ok(())
    }
}
pub fn validate_account(s: &str) -> Result<(), String> {
    if s.is_empty() || s.len() > 64 || s == "." || s == ".." || !s.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c)) {
        Err("account alias must contain 1-64 letters, digits, dots, hyphens or underscores".into())
    } else { Ok(()) }
}
pub(crate) fn unsafe_character(c: char) -> bool { c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') }
pub(crate) fn clean_text(s: &str) -> String { s.chars().filter(|c| !unsafe_character(*c)).take(120).collect() }
