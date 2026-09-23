//! Headless commands shared by the desktop executable and telemetry helper.
//! Display commands use cached data and never query providers.
use crate::{analytics,config,parse_statusline,validate_account,History,Provider,Snapshot,MAX_PAYLOAD_BYTES,SCHEMA_VERSION};
use serde_json::json;
use std::{io::{Read,Write},path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};
pub fn database_path(root:&Path)->PathBuf{root.join("telemetry").join("history.sqlite3")}
pub fn now_ms()->u64{SystemTime::now().duration_since(UNIX_EPOCH).map(|d|d.as_millis() as u64).unwrap_or(0)}
struct Options{command:String,provider:Option<Provider>,account:String,account_explicit:bool,json:bool,limit:usize,bucket:Option<String>,source:Option<String>,reserve:Option<f64>}
fn options(args:&[String])->Result<Options,String>{
    let command=args.first().map(String::as_str).unwrap_or("status");
    if !["status","resets","history","sessions","forecast","ingest","statusline","help","--help"].contains(&command){return Err("unknown telemetry command; use help".into());}
    let mut o=Options{command:command.into(),provider:None,account:"active".into(),account_explicit:false,json:false,limit:200,bucket:None,source:None,reserve:None};let mut i=1;
    while i<args.len(){match args[i].as_str(){
        "--json" if !o.json&&command!="statusline"=>o.json=true,
        "--account" if !o.account_explicit=>{o.account_explicit=true;i+=1;o.account=args.get(i).ok_or("missing account alias")?.clone();validate_account(&o.account)?;},
        "--limit" if command=="history"=>{i+=1;o.limit=args.get(i).ok_or("missing history limit")?.parse().map_err(|_|"invalid history limit")?;if o.limit==0||o.limit>5000{return Err("history limit must be 1-5000".into());}},
        "--source" if ["history","forecast"].contains(&command)&&o.source.is_none()=>{i+=1;let source=args.get(i).ok_or("missing source")?;if !["statusline","legacy_adapter"].contains(&source.as_str()){return Err("invalid telemetry source".into());}o.source=Some(source.clone());},
        "--bucket" if command=="forecast"&&o.bucket.is_none()=>{i+=1;o.bucket=Some(args.get(i).ok_or("missing bucket id")?.clone());},
        "--reserve" if command=="forecast"&&o.reserve.is_none()=>{i+=1;let value=args.get(i).ok_or("missing reserve fraction")?.parse::<f64>().map_err(|_|"invalid reserve fraction")?;if !value.is_finite()||!(0.0..=1.0).contains(&value){return Err("reserve must be between 0 and 1".into());}o.reserve=Some(value);},
        name if !name.starts_with('-')&&o.provider.is_none()=>o.provider=Some(name.parse()?),
        _=>return Err("unknown or misplaced telemetry argument".into()),
    }i+=1;}
    if ["ingest","statusline","history","forecast"].contains(&command)&&o.provider.is_none(){return Err("this command requires a provider".into());}
    if command=="forecast"&&o.bucket.is_none(){return Err("forecast requires --bucket ID".into());}Ok(o)
}
pub fn run(args:&[String],root:&Path,input:&mut dyn Read,out:&mut dyn Write)->Result<(),String>{
    let (args, root) = if args.first().is_some_and(|s|s=="--data-dir") {
        let path=Path::new(args.get(1).ok_or("--data-dir requires an absolute path")?);
        if !path.is_absolute() || args.len()<3 {return Err("--data-dir requires an absolute path followed by a command".into());}
        (&args[2..],path)
    } else {(args,root)};
    if let Some(result)=crate::everywhere::run(args,root,input,out){return result;}
    if let Some(result)=crate::commands::run(args,root,input,out){return result;}
    let o=options(args)?;let now=now_ms();
    if o.command=="help"||o.command=="--help"{return writeln!(out,"Nyrva telemetry\n  [--data-dir ABSOLUTE_PATH] COMMAND\n  top [--once] [--json] [--interval 1..60] [--iterations 1..3600] [--no-ansi]\n  integrations plan|install|remove claude|antigravity [--home PATH] [--executable PATH] [--account ALIAS] [--apply] [--replace]\n  serve [--port 0..65535] [--duration 1..86400]\n  update status | configure --apply | verify --manifest FILE --signature FILE --artifact FILE\n  migrate status | --apply | rollback --apply\n  cockpit | projects | agents | doctor | export [--json]\n  status [PROVIDER] [--account ALIAS] [--json]\n  resets [PROVIDER] [--account ALIAS] [--json]\n  sessions [PROVIDER] [--account ALIAS] [--json]\n  history PROVIDER [--account ALIAS] [--source SOURCE] [--limit 1..5000] [--json]\n  forecast PROVIDER --bucket ID [--account ALIAS] [--source SOURCE] [--reserve 0.1] [--json]\n  ingest antigravity|claude [--account ALIAS] [--json] < payload.json\n  statusline antigravity|claude [--account ALIAS]\n  settings export | import --apply | rollback --apply\n  privacy status | clear --confirm\n  alerts list | refresh\n\nDisplay commands use local cached data. Ingestion/history default to alias active; aliases are not provider authentication.").map_err(|_|"cannot write command output".into());}
    if o.command=="statusline"||o.command=="ingest"{
        let mut bytes=Vec::new();input.take((MAX_PAYLOAD_BYTES+1) as u64).read_to_end(&mut bytes).map_err(|_|"cannot read statusline input")?;
        if bytes.len()>MAX_PAYLOAD_BYTES{return Err("statusline exceeds 256 KiB".into());}
        let raw=std::str::from_utf8(&bytes).map_err(|_|"statusline input is not UTF-8")?;
        let mut snapshot=parse_statusline(o.provider.ok_or("provider required")?,&o.account,raw,now)?;
        let prefs=config::load(root)?;
        if !prefs.providers.enabled(snapshot.provider){return if o.command=="statusline"{writeln!(out,"{}: Nyrva collection disabled",snapshot.provider).map_err(|_|"cannot write statusline".into())}else{write_json(out,&json!({"schema_version":1,"recorded":false,"reason":"provider_disabled"}))};}
        let inserted=config::record(root,&mut snapshot)?;
        if o.command=="statusline"{return writeln!(out,"{}",format_snapshot(&snapshot,now)).map_err(|_|"cannot write statusline".into());}
        return write_json(out,&json!({"schema_version":SCHEMA_VERSION,"recorded":inserted}));
    }
    if o.command=="sessions"{
        let mut sessions=read_sessions(root,now)?;sessions.retain(|s|o.provider.is_none_or(|p|p==s.provider)&&(!o.account_explicit||s.account_id==o.account));
        return write_json(out,&json!({"schema_version":SCHEMA_VERSION,"sessions":crate::observatory::session_views(&sessions,now)}));
    }
    if o.command=="history"||o.command=="forecast"{
        let prefs=config::load(root)?;let provider=o.provider.ok_or("provider required")?;
        let mut samples=if prefs.providers.enabled(provider)&&database_path(root).exists(){History::open_readonly(&database_path(root))?.samples(provider,&o.account,0,o.limit)?}else{Vec::new()};
        samples.retain_mut(|s|prefs.apply(s)&&o.source.as_ref().is_none_or(|source|source==&s.source));
        if o.command=="history"{if o.json{return write_json(out,&json!({"schema_version":SCHEMA_VERSION,"samples":samples}));}for s in samples{writeln!(out,"{} {}",s.observed_at_ms,format_snapshot(&s,now)).map_err(|_|"cannot write history")?;}return Ok(());}
        let bucket=o.bucket.as_deref().ok_or("bucket required")?;
        let selected=samples.last().map(|last|{let reset=last.buckets.iter().find(|b|b.id==bucket).and_then(|b|b.resets_at_ms);samples.iter().filter(|s|s.source==last.source&&s.buckets.iter().any(|b|b.id==bucket&&b.resets_at_ms==reset)).cloned().collect::<Vec<_>>()}).unwrap_or_default();
        let forecast=analytics::forecast(&selected,bucket,now,o.reserve.unwrap_or(prefs.reserve_fraction));return write_json(out,&json!({"schema_version":SCHEMA_VERSION,"forecast":forecast}));
    }
    let mut snapshots=read_current(root,now)?;snapshots.retain(|s|o.provider.is_none_or(|p|p==s.provider)&&(!o.account_explicit||s.account_id==o.account));
    match o.command.as_str(){
        "status"=>{if o.json{return write_json(out,&json!({"schema_version":SCHEMA_VERSION,"captured_at_ms":now,"providers":snapshots}));}if snapshots.is_empty(){writeln!(out,"No local telemetry available; no usage value was inferred.").map_err(|_|"cannot write status")?;}for s in snapshots{writeln!(out,"{}",format_snapshot(&s,now)).map_err(|_|"cannot write status")?;}},
        "resets"=>{let resets=crate::observatory::resets(&snapshots,now);if o.json{return write_json(out,&json!({"schema_version":SCHEMA_VERSION,"resets":resets}));}for r in resets{writeln!(out,"{} [{}] {}: {}",r["provider"].as_str().unwrap_or(""),r["account_id"].as_str().unwrap_or(""),r["bucket_id"].as_str().unwrap_or(""),reset_label(r["resets_at_ms"].as_u64(),now)).map_err(|_|"cannot write resets")?;}},
        _=>return Err("unknown display command".into()),
    }Ok(())
}
pub fn read_sessions(root:&Path,now:u64)->Result<Vec<Snapshot>,String>{crate::observatory::sessions(root,now)}
pub fn read_current(root:&Path,now:u64)->Result<Vec<Snapshot>,String>{crate::observatory::current(root,now)}
pub fn format_snapshot(s:&Snapshot,now:u64)->String{
    let mut parts=vec![format!("{} [{}] {:?}",s.provider,s.account_id,s.effective_status(now))];if let Some(model)=&s.model{parts.push(model.clone());}
    for b in &s.buckets{let value=match(b.remaining_fraction,b.count){(Some(f),_)=>format!("{}{:.0}% left",if b.estimated{"~"}else{""},f*100.0),(_,Some(n))=>format!("count {n}; quota unknown"),_=>"unavailable".into()};parts.push(format!("{} {value} ({})",b.label,reset_label(b.resets_at_ms,now)));}
    if s.buckets.is_empty(){parts.push("quota unavailable".into());}if let Some(pct)=s.context.as_ref().and_then(|c|c.used_percentage){parts.push(format!("context {pct:.0}%"));}if let Some(cost)=s.metrics.as_ref().and_then(|m|m.estimated_cost_usd){parts.push(format!("~USD {cost:.2} session estimate"));}parts.join(" | ")
}
fn reset_label(reset:Option<u64>,now:u64)->String{match reset{None=>"reset unknown".into(),Some(at) if at<=now=>"reset awaiting confirmation".into(),Some(at)=>{let secs=(at-now)/1000;format!("reset in {}h {:02}m {:02}s",secs/3600,(secs%3600)/60,secs%60)}}}
pub(crate) fn write_json(out:&mut dyn Write,value:&serde_json::Value)->Result<(),String>{serde_json::to_writer(&mut *out,value).map_err(|_|"cannot write telemetry JSON")?;writeln!(out).map_err(|_|"cannot finish telemetry output".into())}
