use nyrva_core::{
    analytics::forecast, cli, parse_legacy, parse_statusline, ContextUsage, DataStatus, History,
    Provider, Snapshot,
};
use serde_json::json;
use std::{
    io::Cursor,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
const NOW: u64 = 1_800_000_000_000;
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        Self(std::env::temp_dir().join(format!(
            "nyrva-regression-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn sample(at: u64, remaining: f64) -> Snapshot {
    let mut s = parse_statusline(
        Provider::Antigravity,
        "active",
        &json!({"quota":{"weekly":{"remaining_fraction":remaining}}}).to_string(),
        at,
    )
    .unwrap();
    s.buckets[0].resets_at_ms = Some(NOW + 3_600_000);
    s
}
#[test]
fn deserialized_metadata_cannot_inject_terminal_controls() {
    let mut s = sample(NOW, 0.8);
    s.model = Some("model\u{001b}[2J".into());
    assert!(s.validate().is_err());
    s.model = Some("safe".into());
    s.source_version = Some("x".repeat(1000));
    assert!(s.validate().is_err());
}
#[test]
fn impossible_context_is_rejected_before_serialization() {
    let mut s = sample(NOW, 0.8);
    s.context = Some(ContextUsage {
        used_percentage: Some(120.0),
        size: Some(1000),
        input_tokens: None,
        output_tokens: None,
    });
    assert!(s.validate().is_err());
}
#[test]
fn invalid_legacy_count_does_not_become_a_full_quota() {
    let s=parse_legacy(Provider::Antigravity,"active",&json!({"status":"ok","fetched_at":NOW,"windows":[{"id":"requests","label":"Requests","used":0.0,"count":-1}]})).unwrap();
    assert!(s.buckets.is_empty());
    assert_eq!(s.status, DataStatus::Unavailable);
}
#[test]
fn missing_quota_keeps_last_good_observation_stale_without_retimestamping() {
    let temp = Temp::new();
    let db = History::open(&temp.0.join("telemetry/history.sqlite3")).unwrap();
    db.record(&sample(NOW, 0.8)).unwrap();
    let missing = parse_statusline(Provider::Antigravity, "active", "{}", NOW + 1000).unwrap();
    db.record(&missing).unwrap();
    let latest = db.latest(NOW + 1000).unwrap();
    assert_eq!(latest[0].status, DataStatus::Stale);
    assert_eq!(latest[0].observed_at_ms, NOW);
    assert_eq!(latest[0].buckets[0].remaining_fraction, Some(0.8));
}
#[test]
fn explicit_account_filter_is_respected() {
    let temp = Temp::new();
    for account in ["active", "work"] {
        let args = ["ingest", "antigravity", "--account", account].map(str::to_string);
        cli::run(
            &args,
            &temp.0,
            &mut Cursor::new(br#"{"quota":{"weekly":{"remaining_fraction":0.8}}}"#),
            &mut Vec::new(),
        )
        .unwrap();
    }
    let mut out = Vec::new();
    let args = ["status", "--account", "work", "--json"].map(str::to_string);
    cli::run(&args, &temp.0, &mut Cursor::new(b""), &mut out).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["providers"].as_array().unwrap().len(), 1);
    assert_eq!(v["providers"][0]["account_id"], "work");
}
#[test]
fn reading_later_does_not_shift_the_exhaustion_prediction_forward() {
    let samples = vec![
        sample(NOW - 600_000, 0.9),
        sample(NOW - 300_000, 0.85),
        sample(NOW, 0.8),
    ];
    let a = forecast(&samples, "weekly", NOW, 0.1).unwrap();
    let b = forecast(&samples, "weekly", NOW + 120_000, 0.1).unwrap();
    assert_eq!(a.estimated_exhaustion_at_ms, b.estimated_exhaustion_at_ms);
}
