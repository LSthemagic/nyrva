use nyrva_core::{analytics::forecast, parse_statusline, DataStatus, History, Provider, Snapshot};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
const NOW: u64 = 1_800_000_000_000;
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "nyrva-history-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn db(&self) -> PathBuf {
        self.0.join("history.sqlite3")
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
        "personal",
        &json!({"quota":{"weekly":{"remaining_fraction":remaining}}}).to_string(),
        at,
    )
    .unwrap();
    s.buckets[0].resets_at_ms = Some(NOW + 3_600_000);
    s
}
#[test]
fn history_survives_restart_and_recalculates_freshness() {
    let temp = Temp::new();
    {
        let db = History::open(&temp.db()).unwrap();
        assert!(db.record(&sample(NOW, 0.8)).unwrap());
    }
    let db = History::open_readonly(&temp.db()).unwrap();
    let latest = db.latest(NOW + 700_000).unwrap();
    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].status, DataStatus::Stale);
    assert_eq!(latest[0].buckets[0].remaining_fraction, Some(0.8));
}
#[test]
fn duplicates_and_out_of_order_samples_do_not_replace_latest() {
    let temp = Temp::new();
    let db = History::open(&temp.db()).unwrap();
    let current = sample(NOW, 0.8);
    assert!(db.record(&current).unwrap());
    assert!(!db.record(&current).unwrap());
    assert!(db.record(&sample(NOW - 10_000, 0.9)).unwrap());
    assert_eq!(db.latest(NOW).unwrap()[0].observed_at_ms, NOW);
    assert_eq!(
        db.samples(Provider::Antigravity, "personal", 0, 50)
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn accounts_and_sources_are_not_merged() {
    let temp = Temp::new();
    let db = History::open(&temp.db()).unwrap();
    let a = sample(NOW, 0.8);
    db.record(&a).unwrap();
    let mut b = a.clone();
    b.account_id = "work".into();
    db.record(&b).unwrap();
    b.source = "legacy_adapter".into();
    db.record(&b).unwrap();
    assert_eq!(db.latest(NOW).unwrap().len(), 3);
}
#[test]
fn future_database_schema_is_refused_without_downgrade() {
    let temp = Temp::new();
    {
        let c = rusqlite::Connection::open(temp.db()).unwrap();
        c.execute_batch("PRAGMA user_version=99;").unwrap();
    }
    assert!(History::open(&temp.db()).is_err());
    let c = rusqlite::Connection::open(temp.db()).unwrap();
    assert_eq!(
        c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        99
    );
}
#[test]
fn retention_deletes_only_old_samples_and_readonly_cannot_write() {
    let temp = Temp::new();
    {
        let db = History::open(&temp.db()).unwrap();
        db.record(&sample(NOW - 1000, 0.9)).unwrap();
        db.record(&sample(NOW, 0.8)).unwrap();
        assert_eq!(db.prune(NOW).unwrap(), 1);
    }
    let db = History::open_readonly(&temp.db()).unwrap();
    assert!(db.record(&sample(NOW + 1, 0.7)).is_err());
    assert_eq!(db.latest(NOW).unwrap().len(), 1);
}
#[test]
fn persistence_rejects_invalid_canonical_payloads() {
    let temp = Temp::new();
    let db = History::open(&temp.db()).unwrap();
    let mut s = sample(NOW, 0.8);
    s.buckets[0].remaining_fraction = Some(f64::NAN);
    assert!(db.record(&s).is_err());
    s.buckets[0].remaining_fraction = Some(0.8);
    s.schema_version = 99;
    assert!(db.record(&s).is_err());
}
#[test]
fn forecasts_expose_rate_budget_and_measurement_span() {
    let samples = vec![
        sample(NOW - 600_000, 0.9),
        sample(NOW - 300_000, 0.85),
        sample(NOW, 0.8),
    ];
    let f = forecast(&samples, "weekly", NOW, 0.1).expect("same window with sufficient history");
    assert!((f.fraction_per_hour - 0.6).abs() < 1e-8);
    assert!((f.budget_per_hour - 0.7).abs() < 1e-8);
    assert_eq!(f.sample_count, 3);
    assert_eq!(f.sample_span_ms, 600_000);
    assert!(f.estimated);
    let exhaustion = f.estimated_exhaustion_at_ms.unwrap();
    assert!(exhaustion.abs_diff(NOW + 4_800_000) <= 1);
}
#[test]
fn forecasts_refuse_resets_mixed_accounts_stale_and_insufficient_history() {
    let base = vec![
        sample(NOW - 600_000, 0.9),
        sample(NOW - 300_000, 0.85),
        sample(NOW, 0.8),
    ];
    assert!(forecast(&base[..2], "weekly", NOW, 0.1).is_none());
    assert!(forecast(&base, "weekly", NOW + 700_000, 0.1).is_none());
    assert!(forecast(&base, "weekly", NOW, 1.1).is_none());
    let mut mixed = base.clone();
    mixed[1].account_id = "work".into();
    assert!(forecast(&mixed, "weekly", NOW, 0.1).is_none());
    mixed = base.clone();
    mixed[1].buckets[0].resets_at_ms = Some(NOW + 7200000);
    assert!(forecast(&mixed, "weekly", NOW, 0.1).is_none());
    mixed = base;
    mixed[1].buckets[0].remaining_fraction = Some(0.99);
    assert!(forecast(&mixed, "weekly", NOW, 0.1).is_none());
}
