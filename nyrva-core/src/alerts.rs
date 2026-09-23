//! Transactional threshold notifications; wall-clock passage is never a refill.
use crate::{config,observatory,storage,DataStatus,SCHEMA_VERSION};
use rusqlite::{params,OptionalExtension};
use serde_json::{json,Value};
use std::path::Path;
pub fn refresh(root:&Path,now:u64)->Result<Value,String> {
    let prefs=config::load(root)?;
    if !prefs.alerts.enabled||now<prefs.alerts.quiet_until_ms {return Ok(json!({"schema_version":SCHEMA_VERSION,"new_alerts":[],"silenced":true}));}
    let snapshots=observatory::current(root,now)?;
    if snapshots.is_empty(){return Ok(json!({"schema_version":SCHEMA_VERSION,"new_alerts":[],"silenced":false}));}
    let db=storage::control(root)?;let mut emitted=Vec::new();
    for s in snapshots {if s.status!=DataStatus::Live||!s.is_recent(now){continue;}for b in &s.buckets {
        let Some(fraction)=b.remaining_fraction else{continue};if b.awaiting_reset_confirmation(now){continue;}
        let scope=serde_json::to_string(&(s.provider,s.account_id.as_str(),s.source.as_str(),b.id.as_str())).map_err(|_|"cannot identify alert window")?;
        let old:Option<(u64,Option<u64>,f64,u64)>=db.query_row("SELECT observed,reset_at,fraction,generation FROM alert_windows WHERE scope=?1",[&scope],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|_|"cannot read alert window")?;
        if old.is_some_and(|(at,_,_,_)|s.observed_at_ms<at){continue;}
        let confirmed=old.is_some_and(|(at,reset,last,_)|s.observed_at_ms>at&&reset.is_some()&&b.resets_at_ms.is_some()&&b.resets_at_ms>reset&&fraction>last+0.05);
        let rearmed=old.is_some_and(|(at,_,last,_)|s.observed_at_ms>at&&fraction>last+0.05&&prefs.alerts.thresholds.iter().all(|t|fraction>*t));
        let generation=old.map(|(_,_,_,g)|g+u64::from(confirmed||rearmed)).unwrap_or(0);
        db.execute("INSERT INTO alert_windows(scope,observed,reset_at,fraction,generation) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(scope) DO UPDATE SET observed=excluded.observed,reset_at=excluded.reset_at,fraction=excluded.fraction,generation=excluded.generation",params![scope,s.observed_at_ms,b.resets_at_ms,fraction,generation]).map_err(|_|"cannot update alert window")?;
        let mut kinds:Vec<(&str,Option<f64>)>=prefs.alerts.thresholds.iter().filter(|t|fraction<=**t).map(|t|("quota_threshold",Some(*t))).collect();if confirmed{kinds.push(("quota_reset",None));}
        for (kind,threshold) in kinds {
            // The observed window, not a polling time, identifies a notification.
            let id=serde_json::to_string(&(&scope,b.resets_at_ms,generation,kind,threshold)).map_err(|_|"cannot identify alert")?;
            let alert=json!({"id":id,"kind":kind,"provider":s.provider,"account_id":s.account_id,"source":s.source,"bucket_id":b.id,"bucket_label":b.label,"remaining_fraction":fraction,"threshold":threshold,"resets_at_ms":b.resets_at_ms,"observed_at_ms":s.observed_at_ms,"created_at_ms":now});
            let payload=serde_json::to_string(&alert).map_err(|_|"cannot serialize alert")?;
            if db.execute("INSERT OR IGNORE INTO alert_events(id,at_ms,payload) VALUES (?1,?2,?3)",params![id,now,payload]).map_err(|_|"cannot persist alert")?>0{emitted.push(alert);}
        }
    }}
    db.execute("DELETE FROM alert_events WHERE id IN (SELECT id FROM alert_events ORDER BY at_ms DESC,id LIMIT -1 OFFSET 500)",[]).map_err(|_|"cannot bound alert history")?;
    db.execute("DELETE FROM alert_windows WHERE scope IN (SELECT scope FROM alert_windows ORDER BY observed DESC,scope LIMIT -1 OFFSET 2000)",[]).map_err(|_|"cannot bound alert windows")?;
    storage::commit(&db)?;Ok(json!({"schema_version":SCHEMA_VERSION,"new_alerts":emitted,"silenced":false}))
}
pub fn list(root:&Path)->Result<Value,String>{
    let path=storage::state_dir(root).join("control.sqlite3");if !path.exists(){return Ok(json!({"schema_version":1,"alerts":[]}));}
    storage::check_path(&path)?;let db=rusqlite::Connection::open_with_flags(&path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|_|"cannot read notifications")?;db.busy_timeout(std::time::Duration::from_secs(2)).map_err(|_|"cannot configure notification timeout")?;
    let mut query=db.prepare("SELECT payload FROM alert_events ORDER BY at_ms DESC,id LIMIT 100").map_err(|_|"cannot query notifications")?;let rows=query.query_map([],|r|r.get::<_,String>(0)).map_err(|_|"cannot read notifications")?;let mut alerts=Vec::new();for row in rows{let raw=row.map_err(|_|"cannot read notification")?;if raw.len()>4096{return Err("stored notification exceeds limits".into());}let a:Value=serde_json::from_str(&raw).map_err(|_|"invalid stored notification")?;alerts.push(a);}Ok(json!({"schema_version":1,"alerts":alerts}))
}
