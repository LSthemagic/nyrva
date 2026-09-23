//! These tests exercise the exact headless facade imported by the desktop plugin.
#[path = "../../nyrva/src/observatory_data.rs"]
mod data;
use nyrva_core::{cli, config, parse_statusline, History, Provider};
use std::{path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("nyrva-experience-{}-{}-{}", std::process::id(), cli::now_ms(), N.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&p).unwrap(); Self(p)
    }
}
impl Drop for Root { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
fn observation(root: &Root, now: u64) {
    let s = parse_statusline(Provider::Antigravity, "active", r#"{"quota":{"weekly":{"remaining_fraction":0,"reset_in_seconds":3600}}}"#, now).unwrap();
    config::record(&root.0, s).unwrap();
}
fn query() -> data::HistoryQuery { data::HistoryQuery { provider: "antigravity".into(), account_id: "active".into(), source: "statusline".into(), bucket_id: "weekly".into(), hours: 24 } }
#[test]
fn initial_read_has_the_shared_schema_and_never_creates_storage() {
    let root = Root::new(); let out = data::read(&root.0, cli::now_ms()).unwrap();
    assert_eq!(out["schema_version"], 1); assert!(out["quotas"].as_array().unwrap().is_empty());
    assert!(out["alerts"].as_array().unwrap().is_empty());
    assert_eq!(std::fs::read_dir(&root.0).unwrap().count(), 0);
}
#[test]
fn desktop_history_preserves_zero_and_filters_source_and_account() {
    let root = Root::new(); let now = cli::now_ms(); observation(&root, now);
    let result = data::history(&root.0, &query(), now).unwrap();
    assert_eq!(result["samples"].as_array().unwrap().len(), 1);
    assert_eq!(result["samples"][0]["buckets"][0]["remaining_fraction"], 0.0);
    let mut q = query(); q.source = "legacy_adapter".into();
    assert!(data::history(&root.0, &q, now).unwrap()["samples"].as_array().unwrap().is_empty());
    q.source = "statusline".into(); q.account_id = "work".into();
    assert!(data::history(&root.0, &q, now).unwrap()["samples"].as_array().unwrap().is_empty());
}
#[test]
fn history_rejects_unbounded_ranges_and_unknown_sources() {
    let root = Root::new(); let now = cli::now_ms();
    let mut q = query(); q.hours = 0; assert!(data::history(&root.0, &q, now).is_err());
    q.hours = 24; q.source = "../../credentials".into(); assert!(data::history(&root.0, &q, now).is_err());
    q.source = "statusline".into(); q.account_id = "../auth".into(); assert!(data::history(&root.0, &q, now).is_err());
}
#[test]
fn saved_privacy_preferences_apply_to_current_and_historical_reads() {
    let root = Root::new(); let now = cli::now_ms(); observation(&root, now);
    let mut settings = config::Settings::default(); settings.layout = "reset-first".into(); settings.providers.antigravity = false;
    data::save_settings(&root.0, &settings).unwrap();
    let result = data::read(&root.0, now).unwrap(); assert!(result["quotas"].as_array().unwrap().is_empty());
    assert_eq!(result["settings"]["layout"], "reset-first");
    assert!(data::history(&root.0, &query(), now).unwrap()["samples"].as_array().unwrap().is_empty());
}
#[test]
fn invalid_import_preserves_last_valid_preferences() {
    let root = Root::new(); let mut s = config::Settings::default(); s.retention_days = 30;
    data::save_settings(&root.0, &s).unwrap();
    assert!(data::import_settings(&root.0, r#"{"schema_version":1,"secret":"do not persist"}"#).is_err());
    assert_eq!(config::load(&root.0).unwrap().retention_days, 30);
    assert!(data::import_settings(&root.0, &"x".repeat(32769)).is_err());
}
#[test]
fn clear_requires_exact_confirmation_and_preserves_provider_files_and_preferences() {
    let root = Root::new(); let now = cli::now_ms(); observation(&root, now);
    std::fs::write(root.0.join("provider-owned.json"), b"SYNTHETIC PROVIDER SENTINEL").unwrap();
    data::save_settings(&root.0, &config::Settings::default()).unwrap();
    assert!(data::clear_history(&root.0, "yes").is_err());
    assert_eq!(History::open_readonly(&cli::database_path(&root.0)).unwrap().latest(now).unwrap().len(), 1);
    data::clear_history(&root.0, "CLEAR NYRVA").unwrap();
    assert!(History::open_readonly(&cli::database_path(&root.0)).unwrap().latest(now).unwrap().is_empty());
    assert_eq!(std::fs::read(root.0.join("provider-owned.json")).unwrap(), b"SYNTHETIC PROVIDER SENTINEL");
    assert_eq!(config::load(&root.0).unwrap().retention_days, 90);
}
