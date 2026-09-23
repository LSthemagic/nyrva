//! Control-plane commands. Read commands never create provider or history files.
use crate::{alerts,cli,config,observatory,storage,History,MAX_PAYLOAD_BYTES};
use serde_json::{json,Value};
use std::{io::{Read,Write},path::Path};
pub(crate) fn run(args:&[String],root:&Path,input:&mut dyn Read,out:&mut dyn Write)->Option<Result<(),String>>{
    let command=args.first()?.as_str();if !["cockpit","projects","agents","settings","privacy","alerts","doctor","export"].contains(&command){return None;}
    Some((||{let now=cli::now_ms();let action=args.get(1).filter(|s|!s.starts_with('-')).map(String::as_str);let value=match command{
        "cockpit"|"projects"|"agents"|"doctor"|"export"=>{only(args,&["--json"])?;match command{"cockpit"=>observatory::cockpit(root,now)?,"projects"=>json!({"schema_version":1,"projects":observatory::projects(&observatory::sessions(root,now)?)}),"agents"=>json!({"schema_version":1,"agents":observatory::agents(&observatory::sessions(root,now)?,now)}),_=>observatory::diagnostics(root,now)}},
        "settings"=>match action{
            Some("export")=>{only(&args[1..],&["--json"])?;json!({"schema_version":1,"settings":config::load(root)?})},
            Some("import")=>{only(&args[1..],&["--apply","--json"])?;require(args,"--apply","settings import requires --apply")?;let raw=bounded(input)?;json!({"schema_version":1,"settings":config::import(root,&raw)?})},
            Some("rollback")=>{only(&args[1..],&["--apply","--json"])?;require(args,"--apply","settings rollback requires --apply")?;json!({"schema_version":1,"settings":config::rollback(root)?})},_=>return Err("settings requires export, import --apply or rollback --apply".into()),
        },
        "alerts"=>match action{Some("refresh")=>{only(&args[1..],&["--json"])?;alerts::refresh(root,now)?},None|Some("list")=>{let rest=if action.is_some(){&args[1..]}else{args};only(rest,&["--json"])?;alerts::list(root)?},_=>return Err("alerts requires list or refresh".into())},
        "privacy"=>match action{
            Some("clear")=>{only(&args[1..],&["--confirm","--json"])?;require(args,"--confirm","privacy clear requires --confirm")?;clear(root)?;json!({"schema_version":1,"cleared":true,"scope":"nyrva_history_and_alerts","provider_credentials_touched":false})},
            None|Some("status")=>{let rest=if action.is_some(){&args[1..]}else{args};only(rest,&["--json"])?;let prefs=config::load(root)?;json!({"schema_version":1,"retention_days":prefs.retention_days,"collect_session_metadata":prefs.collect_session_metadata,"providers":prefs.providers,"persisted_fields":["provider","local_account_alias","source","observation_time","independent_quota_buckets","optional_session_metadata"],"never_persisted":["prompts","responses","tool_commands","credentials","email","absolute_project_paths"],"clear_scope":"Nyrva SQLite history and notifications only; provider files and preferences are preserved"})},_=>return Err("privacy requires status or clear --confirm".into())},
        _=>unreachable!(),
    };cli::write_json(out,&value)})())
}
pub(crate) fn only(args:&[String],allowed:&[&str])->Result<(),String>{let mut seen=std::collections::HashSet::new();for a in args.iter().skip(1){if !allowed.contains(&a.as_str())||!seen.insert(a){return Err("unknown, duplicate or misplaced argument".into());}}Ok(())}
pub(crate) fn require(args:&[String],flag:&str,message:&str)->Result<(),String>{if args.iter().any(|s|s==flag){Ok(())}else{Err(message.into())}}
pub(crate) fn bounded(input:&mut dyn Read)->Result<Vec<u8>,String>{let mut raw=Vec::new();input.take((MAX_PAYLOAD_BYTES+1) as u64).read_to_end(&mut raw).map_err(|_|"cannot read command input")?;if raw.len()>MAX_PAYLOAD_BYTES{return Err("command input exceeds 256 KiB".into());}Ok(raw)}
pub fn clear(root:&Path)->Result<(),String>{
    let control=storage::control(root)?;let path=cli::database_path(root);
    if path.exists(){let _validated=History::open(&path)?;let db=rusqlite::Connection::open(&path).map_err(|_|"cannot open Nyrva history for clearing")?;db.busy_timeout(std::time::Duration::from_secs(5)).map_err(|_|"cannot configure clearing timeout")?;db.execute_batch("PRAGMA secure_delete=ON; BEGIN IMMEDIATE; DELETE FROM observations; COMMIT; PRAGMA wal_checkpoint(TRUNCATE); VACUUM;").map_err(|_|"cannot finish clearing Nyrva history")?;}
    control.execute_batch("DELETE FROM alert_events; DELETE FROM alert_windows;").map_err(|_|"cannot clear Nyrva notifications")?;storage::commit(&control)?;Ok(())
}
