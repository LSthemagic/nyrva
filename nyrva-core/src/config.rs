//! Portable preferences contain no credentials, paths, commands or API session tokens.
use crate::{storage, validate_account, History, Provider, Snapshot, MAX_TIMESTAMP_MS};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
const LIMIT:usize=32*1024;
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Providers { pub claude:bool,pub codex:bool,pub cursor:bool,pub antigravity:bool }
impl Default for Providers { fn default()->Self { Self{claude:true,codex:true,cursor:true,antigravity:true} } }
impl Providers { pub fn enabled(&self,p:Provider)->bool { match p { Provider::Claude=>self.claude,Provider::Codex=>self.codex,Provider::Cursor=>self.cursor,Provider::Antigravity=>self.antigravity } } }
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlertSettings { pub enabled:bool,pub thresholds:Vec<f64>,pub quiet_until_ms:u64 }
impl Default for AlertSettings { fn default()->Self { Self{enabled:true,thresholds:vec![0.2,0.1],quiet_until_ms:0} } }
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub schema_version:u32,pub layout:String,pub retention_days:u16,pub collect_session_metadata:bool,
    pub providers:Providers,pub alerts:AlertSettings,pub reserve_fraction:f64,
    #[serde(default)] pub account_aliases:BTreeMap<String,String>,
}
impl Default for Settings { fn default()->Self { Self{schema_version:1,layout:"compact".into(),retention_days:90,collect_session_metadata:true,providers:Providers::default(),alerts:AlertSettings::default(),reserve_fraction:0.1,account_aliases:BTreeMap::new()} } }
impl Settings {
    pub fn validate(&self)->Result<(),String> {
        if self.schema_version!=1 { return Err("unsupported preferences schema; no preferences changed".into()); }
        if !["compact","bars","reset-first"].contains(&self.layout.as_str()) { return Err("layout must be compact, bars or reset-first".into()); }
        if !(1..=365).contains(&self.retention_days) { return Err("retention_days must be 1-365".into()); }
        if !self.reserve_fraction.is_finite() || !(0.0..=1.0).contains(&self.reserve_fraction) { return Err("invalid reserve fraction".into()); }
        if self.alerts.thresholds.is_empty() || self.alerts.thresholds.len()>8 || self.alerts.thresholds.iter().any(|v|!v.is_finite()||*v<=0.0||*v>1.0) || self.alerts.quiet_until_ms>MAX_TIMESTAMP_MS { return Err("invalid alert preferences".into()); }
        let mut seen=std::collections::HashSet::new(); for n in &self.alerts.thresholds { if !seen.insert(n.to_bits()) { return Err("duplicate alert threshold".into()); } }
        if self.account_aliases.len()>128 { return Err("too many account aliases".into()); }
        for (key,value) in &self.account_aliases { validate_account(key)?; validate_account(value)?; }
        Ok(())
    }
    pub fn apply(&self,s:&mut Snapshot)->bool {
        if !self.collect_session_metadata { s.session=None;s.metrics=None;s.context=None;s.model=None; }
        self.providers.enabled(s.provider)
    }
}
pub fn load(root:&Path)->Result<Settings,String> {
    match storage::read_optional(&storage::state_dir(root).join("preferences.json"),LIMIT)? {
        None=>Ok(Settings::default()),Some(bytes)=>decode(&bytes),
    }
}
fn decode(bytes:&[u8])->Result<Settings,String> { if bytes.len()>LIMIT { return Err("preferences exceed 32 KiB".into()); } let s:Settings=serde_json::from_slice(bytes).map_err(|_|"invalid preferences or unknown fields")?; s.validate()?; Ok(s) }
pub fn import(root:&Path,bytes:&[u8])->Result<Settings,String> {
    let next=decode(bytes)?; let db=storage::control(root)?; let old=load(root)?;
    let dir=storage::state_dir(root);
    storage::atomic_write(&dir.join("preferences.previous.json"),&serde_json::to_vec_pretty(&old).map_err(|_|"cannot encode preferences")?)?;
    storage::atomic_write(&dir.join("preferences.json"),&serde_json::to_vec_pretty(&next).map_err(|_|"cannot encode preferences")?)?;
    storage::commit(&db)?; Ok(next)
}
pub fn rollback(root:&Path)->Result<Settings,String> {
    let db=storage::control(root)?; let dir=storage::state_dir(root); let previous=storage::read_optional(&dir.join("preferences.previous.json"),LIMIT)?.ok_or("no previous preferences to restore")?; let previous=decode(&previous)?; let current=load(root)?;
    storage::atomic_write(&dir.join("preferences.json"),&serde_json::to_vec_pretty(&previous).map_err(|_|"cannot encode preferences")?)?;
    storage::atomic_write(&dir.join("preferences.previous.json"),&serde_json::to_vec_pretty(&current).map_err(|_|"cannot encode preferences")?)?;
    storage::commit(&db)?; Ok(previous)
}
/// The one ingestion boundary used by statusline helpers and desktop adapters.
pub fn record(root:&Path,s:&mut Snapshot)->Result<bool,String> {
    let settings=load(root)?;
    if !settings.apply(s) { return Ok(false); }
    let db=History::open(&crate::cli::database_path(root))?;
    let recorded=db.record(s)?;
    db.prune(crate::cli::now_ms().saturating_sub(u64::from(settings.retention_days)*86_400_000))?;
    Ok(recorded)
}
