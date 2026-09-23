//! Contracts for the next Core slice. All provider data here is synthetic.
use nyrva_core::{cli, parse_statusline, DataStatus, History, Provider};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("nyrva-session-contract-{}-{}-{}", std::process::id(), cli::now_ms(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&p).unwrap(); Self(p)
    }
}
impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn query(root: &std::path::Path, args: &[&str]) -> Value {
    let args = args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut output = Vec::new();
    cli::run(&args, root, &mut std::io::empty(), &mut output).unwrap();
    serde_json::from_slice(&output).unwrap()
}
fn ag(id: &str, now: u64, quota: bool) -> nyrva_core::Snapshot {
    let mut v = json!({"session_id":id,"model":{"id":"fixture-model"},"workspace":{"project_dir":"/private/work/project-a"},"agent_state":"working"});
    if quota { v["quota"] = json!({"weekly":{"remaining_fraction":0.6,"reset_in_seconds":3600}}); }
    parse_statusline(Provider::Antigravity,"active",&v.to_string(),now).unwrap()
}

#[test]
fn claude_statusline_supports_documented_quota_context_and_estimated_cost() {
    let now = cli::now_ms();
    let raw = json!({"session_id":"claude-one","model":{"id":"fixture-model"},"effort":{"level":"high"},"cost":{"total_cost_usd":1.25,"total_duration_ms":12000},"context_window":{"used_percentage":42.5,"context_window_size":200000},"rate_limits":{"five_hour":{"used_percentage":25.0,"resets_at":now/1000+3600},"seven_day":{"used_percentage":80.0,"resets_at":now/1000+86400}},"email":"secret@example.invalid","prompt":"private text","api_key":"not-to-store"});
    let parsed = parse_statusline(Provider::Claude,"work",&raw.to_string(),now);
    assert!(parsed.is_ok(),"documented Claude statusline must be accepted");
    let s = parsed.unwrap();
    assert_eq!(s.status,DataStatus::Live);
    assert_eq!(s.buckets.len(),2);
    let b = s.buckets.iter().find(|b| b.id=="five_hour").unwrap();
    assert_eq!(b.remaining_fraction,Some(0.75));
    assert_eq!(b.resets_at_ms,Some((now/1000+3600)*1000));
    let serialized = serde_json::to_value(&s).unwrap();
    assert_eq!(serialized["metrics"]["estimated_cost_usd"],json!(1.25));
    assert_eq!(serialized["metrics"]["effort"],json!("high"));
    let text = serialized.to_string();
    for secret in ["secret@example.invalid","private text","not-to-store"] { assert!(!text.contains(secret)); }
}

#[test]
fn conversation_id_takes_precedence_over_compatibility_alias() {
    let s = parse_statusline(Provider::Antigravity,"active",r#"{"conversation_id":"canonical","session_id":"compat","agent_state":"idle"}"#,cli::now_ms()).unwrap();
    assert_eq!(s.session.unwrap().id.as_deref(),Some("canonical"));
}

#[test]
fn sessions_from_the_same_account_and_source_do_not_overwrite_each_other() {
    let t = Temp::new(); let now = cli::now_ms();
    let db = History::open(&cli::database_path(&t.0)).unwrap();
    assert!(db.record(&ag("one",now,true)).unwrap());
    assert!(db.record(&ag("two",now,true)).unwrap(),"distinct sessions at one timestamp must both survive");
    assert!(!db.record(&ag("two",now,true)).unwrap(),"same observation is idempotent");
    drop(db);
    let v = query(&t.0,&["sessions","antigravity","--json"]);
    let rows = v["sessions"].as_array().unwrap();
    assert_eq!(rows.len(),2,"sessions are not the account's latest quota snapshot");
    let ids = rows.iter().map(|s| s["session"]["id"].as_str().unwrap()).collect::<std::collections::HashSet<_>>();
    assert!(ids.contains("one") && ids.contains("two"));
}

#[test]
fn missing_quota_does_not_replace_fresh_session_metadata_with_old_metadata() {
    let t = Temp::new(); let now = cli::now_ms();
    let db = History::open(&cli::database_path(&t.0)).unwrap();
    db.record(&ag("one",now-1000,true)).unwrap();
    let mut current = ag("one",now,false);
    current.model = Some("new-model".into());
    current.session.as_mut().unwrap().state = Some("done".into());
    db.record(&current).unwrap(); drop(db);
    let v = query(&t.0,&["sessions","--json"]);
    let rows = v["sessions"].as_array().unwrap();
    assert_eq!(rows.len(),1);
    assert_eq!(rows[0]["model"],json!("new-model"));
    assert_eq!(rows[0]["session"]["state"],json!("done"));
    assert_eq!(rows[0]["observed_at_ms"],json!(now));
    // Quota recovery remains conservative and keeps the real quota timestamp.
    let quotas = query(&t.0,&["status","--json"]);
    assert_eq!(quotas["providers"][0]["observed_at_ms"],json!(now-1000));
    assert_eq!(quotas["providers"][0]["status"],json!("stale"));
}

#[test]
fn invalid_cost_does_not_turn_into_zero_or_real_billing() {
    let parsed = parse_statusline(Provider::Claude,"active",r#"{"session_id":"one","cost":{"total_cost_usd":-3}}"#,cli::now_ms());
    assert!(parsed.is_ok());
    let v = serde_json::to_value(parsed.unwrap()).unwrap();
    assert!(v["metrics"]["estimated_cost_usd"].is_null());
    assert!(v["metrics"]["billed_cost_usd"].is_null());
}

#[test]
fn malformed_claude_percentages_and_unknown_windows_are_not_fabricated() {
    let parsed = parse_statusline(Provider::Claude,"active",r#"{"rate_limits":{"five_hour":{"used_percentage":120,"resets_at":2000000000},"seven_day":{"used_percentage":0,"resets_at":2000000000},"unknown_bucket":{"used_percentage":30}}}"#,cli::now_ms());
    assert!(parsed.is_ok());
    let s = parsed.unwrap();
    assert_eq!(s.buckets.len(),1);
    assert_eq!(s.buckets[0].id,"seven_day");
    assert_eq!(s.buckets[0].remaining_fraction,Some(1.0));
}

#[test]
fn existing_v1_database_is_migrated_without_losing_observations() {
    let t = Temp::new(); let now = cli::now_ms();
    let path = cli::database_path(&t.0); fs::create_dir_all(path.parent().unwrap()).unwrap();
    let old = rusqlite::Connection::open(&path).unwrap();
    old.execute_batch("CREATE TABLE observations(provider TEXT NOT NULL,account_id TEXT NOT NULL,source TEXT NOT NULL,observed_at_ms INTEGER NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(provider,account_id,source,observed_at_ms)) WITHOUT ROWID; CREATE INDEX observations_time ON observations(observed_at_ms); PRAGMA user_version=1;").unwrap();
    old.execute("INSERT INTO observations VALUES (?1,?2,?3,?4,?5)",rusqlite::params!["antigravity","active","statusline",now,serde_json::to_string(&ag("old",now,true)).unwrap()]).unwrap(); drop(old);
    let db = History::open(&path).unwrap();
    assert!(db.record(&ag("new",now,true)).unwrap());
    assert_eq!(db.samples(Provider::Antigravity,"active",0,100).unwrap().len(),2);
}
