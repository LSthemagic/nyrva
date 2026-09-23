use nyrva_core::{cli,parse_statusline,History,Provider};
use serde_json::{json,Value};
use std::{fs,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
static NEXT:AtomicU64=AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp { fn new()->Self { let p=std::env::temp_dir().join(format!("nyrva-cockpit-{}-{}-{}",std::process::id(),cli::now_ms(),NEXT.fetch_add(1,Ordering::Relaxed)));fs::create_dir_all(&p).unwrap();Self(p) } }
impl Drop for Temp {fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
fn call(t:&Temp,args:&[&str],input:Value)->Result<Value,String>{let mut out=Vec::new();let args=args.iter().map(|s|s.to_string()).collect::<Vec<_>>();let data=input.to_string();cli::run(&args,&t.0,&mut data.as_bytes(),&mut out)?;serde_json::from_slice(&out).map_err(|_|"output must be JSON".into())}
fn get(t:&Temp,args:&[&str])->Value {let r=call(t,args,Value::Null);assert!(r.is_ok(),"command failed: {r:?}");r.unwrap()}
fn record(t:&Temp,id:&str,cost:f64,at:u64){let s=parse_statusline(Provider::Claude,"active",&json!({"session_id":id,"workspace":{"project_dir":"/private/monorepo"},"cost":{"total_cost_usd":cost,"total_duration_ms":1000}}).to_string(),at).unwrap();History::open(&cli::database_path(&t.0)).unwrap().record(&s).unwrap();}
fn quota(t:&Temp,f:f64,reset:u64,at:u64){let mut s=parse_statusline(Provider::Antigravity,"active",&json!({"quota":{"weekly":{"remaining_fraction":f}}}).to_string(),at).unwrap();s.buckets[0].resets_at_ms=Some(reset);History::open(&cli::database_path(&t.0)).unwrap().record(&s).unwrap();}

#[test]
fn empty_cockpit_has_honest_states_and_does_not_create_history(){let t=Temp::new();let v=get(&t,&["cockpit","--json"]);assert_eq!(v["schema_version"],1);assert_eq!(v["providers"].as_array().unwrap().len(),4);assert_eq!(v["pulse"]["state"],"unknown");assert!(v["pulse"]["capacity_percentage"].is_null());assert!(v["sessions"].as_array().unwrap().is_empty());assert!(!cli::database_path(&t.0).exists());}

#[test]
fn project_costs_count_each_session_once_and_never_claim_real_billing(){let t=Temp::new();let now=cli::now_ms();record(&t,"one",1.0,now-2000);record(&t,"one",1.5,now-1000);record(&t,"two",1.0,now);let v=get(&t,&["projects","--json"]);let p=&v["projects"][0];assert_eq!(p["session_count"],2);assert_eq!(p["estimated_cost_usd"],2.5);assert_eq!(p["sessions_with_cost"],2);assert!(p["billed_cost_usd"].is_null());assert!(!v.to_string().contains("/private"));}

#[test]
fn preferences_import_is_validated_explicit_and_excludes_sensitive_fields(){let t=Temp::new();let original=get(&t,&["settings","export","--json"]);let mut settings=original["settings"].clone();settings["layout"]=json!("bars");assert!(call(&t,&["settings","import"],settings.clone()).is_err());assert!(call(&t,&["settings","import","--apply"],settings.clone()).is_ok());assert_eq!(get(&t,&["settings","export","--json"])["settings"]["layout"],"bars");settings["retention_days"]=json!(0);assert!(call(&t,&["settings","import","--apply"],settings).is_err());let mut unknown=original["settings"].clone();unknown["api_token"]=json!("secret");assert!(call(&t,&["settings","import","--apply"],unknown).is_err());assert_eq!(get(&t,&["settings","export","--json"])["settings"]["layout"],"bars");}

#[test]
fn privacy_clear_requires_confirmation_and_only_clears_nyrva_history(){let t=Temp::new();let sentinel=t.0.join("provider-credentials.json");fs::write(&sentinel,"do-not-touch").unwrap();record(&t,"one",2.0,cli::now_ms());assert!(call(&t,&["privacy","clear"],Value::Null).is_err());assert_eq!(get(&t,&["sessions","--json"])["sessions"].as_array().unwrap().len(),1);get(&t,&["privacy","clear","--confirm"]);assert!(get(&t,&["sessions","--json"])["sessions"].as_array().unwrap().is_empty());assert_eq!(fs::read_to_string(sentinel).unwrap(),"do-not-touch");}

#[test]
fn privacy_disables_session_metadata_at_the_ingestion_boundary(){let t=Temp::new();let mut settings=get(&t,&["settings","export","--json"])["settings"].clone();settings["collect_session_metadata"]=json!(false);call(&t,&["settings","import","--apply"],settings).unwrap();let payload=json!({"session_id":"not-to-store","workspace":{"project_dir":"/private/repo"},"model":{"id":"model-private"},"context_window":{"used_percentage":42},"quota":{"weekly":{"remaining_fraction":0.5}}});call(&t,&["ingest","antigravity","--json"],payload).unwrap();let v=get(&t,&["status","--json"]);let text=v.to_string();assert!(!text.contains("not-to-store")&&!text.contains("model-private"));assert_eq!(v["providers"][0]["buckets"][0]["remaining_fraction"],0.5);assert!(get(&t,&["sessions","--json"])["sessions"].as_array().unwrap().is_empty());}

#[test]
fn disabled_provider_cannot_be_reenabled_by_a_statusline_process(){let t=Temp::new();let mut settings=get(&t,&["settings","export","--json"])["settings"].clone();settings["providers"]["antigravity"]=json!(false);call(&t,&["settings","import","--apply"],settings).unwrap();let r=call(&t,&["ingest","antigravity","--json"],json!({"quota":{"weekly":{"remaining_fraction":0.5}}}));assert!(r.is_ok());assert_eq!(r.unwrap()["recorded"],false);assert!(!cli::database_path(&t.0).exists());}

#[test]
fn alerts_are_deduplicated_and_clock_passage_is_not_a_confirmed_reset(){let t=Temp::new();let now=cli::now_ms();quota(&t,0.09,now+3600000,now-1000);let first=get(&t,&["alerts","refresh","--json"]);assert!(!first["new_alerts"].as_array().unwrap().is_empty());let repeated=get(&t,&["alerts","refresh","--json"]);assert!(repeated["new_alerts"].as_array().unwrap().is_empty());quota(&t,0.09,now-500,now);let third=get(&t,&["alerts","refresh","--json"]);assert!(!third.to_string().contains("quota_reset"),"elapsed wall clock alone must never announce a refill");}

#[test]
fn a_broken_provider_cache_does_not_hide_other_providers(){let t=Temp::new();let now=cli::now_ms();quota(&t,0.75,now+3600000,now);fs::write(t.0.join("codex.json"),"{broken").unwrap();let v=get(&t,&["cockpit","--json"]);let quotas=v["quotas"].as_array().unwrap();assert!(quotas.iter().any(|q|q["provider"]=="antigravity"&&q["buckets"][0]["remaining_fraction"]==0.75));let providers=v["providers"].as_array().unwrap();assert!(providers.iter().any(|p|p["id"]=="codex"&&p["status"]=="error"));}
