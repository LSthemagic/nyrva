//! Headless, bounded desktop facade over the shared core. No provider credentials,
//! filesystem paths from IPC, network requests, or presentation-derived measurements.
use nyrva_core::{alerts, analytics, cli, commands, config, observatory, validate_account, History, Provider};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::Path, str::FromStr};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryQuery {
    pub provider: String,
    pub account_id: String,
    pub source: String,
    pub bucket_id: String,
    pub hours: u16,
}

pub fn read(root: &Path, now: u64) -> Result<Value, String> {
    let mut result = observatory::cockpit(root, now)?;
    match alerts::list(root) {
        Ok(list) => {
            result["alerts"] = list.get("alerts").cloned().unwrap_or_else(|| json!([]));
            result["alerts_error"] = Value::Null;
        }
        Err(_) => {
            result["alerts"] = json!([]);
            result["alerts_error"] = json!("local_alerts_unavailable");
        }
    }
    Ok(result)
}

pub fn history(root: &Path, query: &HistoryQuery, now: u64) -> Result<Value, String> {
    let provider = Provider::from_str(&query.provider)?;
    validate_account(&query.account_id)?;
    if !(1..=8760).contains(&query.hours)
        || !["statusline", "legacy_adapter"].contains(&query.source.as_str())
        || query.bucket_id.is_empty()
        || query.bucket_id.len() > 512
        || query.bucket_id.chars().any(|c| c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
        || now > nyrva_core::MAX_TIMESTAMP_MS
    {
        return Err("invalid or unbounded history query".into());
    }
    let settings = config::load(root)?;
    let path = cli::database_path(root);
    if !settings.providers.enabled(provider) || !path.try_exists().map_err(|_| "cannot inspect local history")? {
        return Ok(json!({"schema_version":1,"samples":[],"forecast":null,"truncated":false}));
    }
    let hours = u64::from(query.hours).min(u64::from(settings.retention_days) * 24);
    let mut samples = History::open_readonly(&path)?.samples(provider, &query.account_id, now.saturating_sub(hours * 3_600_000), 501)?;
    // The shared query caps rows before filtering sources; expose that limitation.
    let truncated = samples.len() > 500;
    if truncated { samples.remove(0); }
    samples.retain_mut(|s| {
        if s.source != query.source || !settings.apply(s) { return false; }
        s.buckets.retain(|b| b.id == query.bucket_id);
        // Historical quota charts do not need or expose session metadata.
        s.session = None; s.metrics = None; s.context = None; s.model = None;
        !s.buckets.is_empty()
    });
    let forecast = samples.last().and_then(|last| {
        let reset = last.buckets.first()?.resets_at_ms;
        let window = last.buckets.first()?.window_seconds;
        let compatible: Vec<_> = samples.iter().filter(|s| s.buckets.first().is_some_and(|b| b.resets_at_ms == reset && b.window_seconds == window)).cloned().collect();
        analytics::forecast(&compatible, &query.bucket_id, now, settings.reserve_fraction)
    });
    Ok(json!({"schema_version":1,"samples":samples,"forecast":forecast,"truncated":truncated}))
}

pub fn save_settings(root: &Path, settings: &config::Settings) -> Result<config::Settings, String> {
    settings.validate()?;
    let bytes = serde_json::to_vec(settings).map_err(|_| "cannot encode preferences")?;
    config::import(root, &bytes)
}

pub fn import_settings(root: &Path, text: &str) -> Result<config::Settings, String> {
    if text.len() > 32 * 1024 { return Err("preferences exceed 32 KiB".into()); }
    config::import(root, text.as_bytes())
}

pub fn clear_history(root: &Path, confirmation: &str) -> Result<(), String> {
    if confirmation != "CLEAR NYRVA" { return Err("explicit local history confirmation required".into()); }
    commands::clear(root)
}
