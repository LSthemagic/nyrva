use nyrva_core::cli;
use std::{io::Cursor, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};
struct Temp(PathBuf);
impl Temp { fn new() -> Self { static N: AtomicU64 = AtomicU64::new(0); let p = std::env::temp_dir().join(format!("nyrva-cli-{}-{}",std::process::id(),N.fetch_add(1,Ordering::Relaxed))); Self(p) } }
impl Drop for Temp { fn drop(&mut self) { let _=std::fs::remove_dir_all(&self.0); } }
fn run(t: &Temp, args: &[&str], input: &str) -> Result<String, String> {
    let mut out=Vec::new(); let args=args.iter().map(|s|s.to_string()).collect::<Vec<_>>();
    cli::run(&args, &t.0, &mut Cursor::new(input.as_bytes()), &mut out)?;
    String::from_utf8(out).map_err(|_|"invalid UTF-8 output".into())
}
#[test]
fn statusline_to_sqlite_to_json_is_a_real_end_to_end_flow() {
    let t=Temp::new();
    let output=run(&t,&["statusline","antigravity","--account","work"],r#"{"quota":{"weekly":{"remaining_fraction":0.75,"reset_in_seconds":3600}},"access_token":"TOP_SECRET","prompt":"CONFIDENTIAL"}"#).unwrap();
    assert!(output.contains("75%")); assert!(output.contains("weekly"));
    let output=run(&t,&["status","--json"],"").unwrap();
    let v:serde_json::Value=serde_json::from_str(&output).unwrap();
    assert_eq!(v["schema_version"],1);
    assert_eq!(v["providers"][0]["account_id"],"work");
    assert_eq!(v["providers"][0]["buckets"][0]["remaining_fraction"],0.75);
    assert!(!output.contains("TOP_SECRET")); assert!(!output.contains("CONFIDENTIAL"));
    let output=run(&t,&["resets","--json"],"").unwrap();
    let v:serde_json::Value=serde_json::from_str(&output).unwrap();
    assert_eq!(v["resets"].as_array().unwrap().len(),1);
}
#[test]
fn readonly_status_does_not_create_a_database_or_guess_usage() {
    let t=Temp::new(); let output=run(&t,&["status","--json"],"").unwrap();
    let v:serde_json::Value=serde_json::from_str(&output).unwrap();
    assert_eq!(v["providers"].as_array().unwrap().len(),0); assert!(!t.0.exists());
}
#[test]
fn malformed_statusline_and_unknown_flags_fail_without_echoing_input() {
    let t=Temp::new();
    let err=run(&t,&["statusline","antigravity"],"SECRET BAD JSON").unwrap_err();
    assert!(!err.contains("SECRET"));
    assert!(run(&t,&["status","--wat"],"").is_err());
    assert!(run(&t,&["statusline","antigravity","--account","../bad"],"{}").is_err());
    assert!(!t.0.exists());
}
