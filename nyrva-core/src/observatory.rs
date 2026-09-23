//! Shared read models for desktop, terminal and local API. No provider queries.
use crate::{cli,config,storage,parse_legacy,DataStatus,History,Provider,Snapshot,MAX_PAYLOAD_BYTES,SCHEMA_VERSION};
use serde_json::{json,Value};
use std::{collections::BTreeMap,path::Path};
pub const PROVIDERS:[Provider;4]=[Provider::Claude,Provider::Codex,Provider::Cursor,Provider::Antigravity];
pub fn current(root:&Path,now:u64)->Result<Vec<Snapshot>,String> {
    let prefs=config::load(root)?; let path=cli::database_path(root);
    let mut snapshots=if path.exists(){History::open_readonly(&path)?.latest(now)?}else{vec![]};
    snapshots.retain_mut(|s|prefs.apply(s));
    for (provider,name) in [(Provider::Claude,"usage.json"),(Provider::Codex,"codex.json"),(Provider::Cursor,"cursor.json"),(Provider::Antigravity,"antigravity.json")] {
        if !prefs.providers.enabled(provider){continue;}
        let result=(||{let bytes=match storage::read_optional(&root.join(name),MAX_PAYLOAD_BYTES)?{Some(b)=>b,None=>return Ok(None)};let raw=serde_json::from_slice(&bytes).map_err(|_|"invalid provider cache JSON")?;let mut s=parse_legacy(provider,"active",&raw)?;s.status=s.effective_status(now);prefs.apply(&mut s);Ok::<_,String>(Some(s))})();
        let s=match result { Ok(None)=>continue,Ok(Some(s))=>s,Err(_)=>{let mut s=Snapshot::empty(provider,"active","legacy_adapter",0);s.status=DataStatus::Error;s} };
        if let Some(old)=snapshots.iter_mut().find(|x|x.provider==provider&&x.account_id=="active"&&x.source=="legacy_adapter") {
            if s.status==DataStatus::Error { old.status=DataStatus::Error; } else if s.observed_at_ms>=old.observed_at_ms { *old=s; }
        } else { snapshots.push(s); }
    }
    snapshots.sort_by(|a,b|(a.provider.as_str(),&a.account_id,&a.source).cmp(&(b.provider.as_str(),&b.account_id,&b.source))); Ok(snapshots)
}
pub fn sessions(root:&Path,now:u64)->Result<Vec<Snapshot>,String> {
    let prefs=config::load(root)?; if !prefs.collect_session_metadata||!cli::database_path(root).exists(){return Ok(vec![]);}
    let mut s=History::open_readonly(&cli::database_path(root))?.sessions(now)?;s.retain_mut(|x|prefs.apply(x));Ok(s)
}
pub fn session_views(samples:&[Snapshot],now:u64)->Vec<Value> { samples.iter().map(|s|json!({"provider":s.provider,"account_id":s.account_id,"source":s.source,"status":s.status,"session_status":if s.is_recent(now){"recent"}else{"stale"},"observed_at_ms":s.observed_at_ms,"model":s.model,"context":s.context,"session":s.session,"metrics":s.metrics})).collect() }
pub fn projects(samples:&[Snapshot])->Vec<Value> {
    // Deduplicate cumulative totals even if a future adapter reports the same session.
    let mut latest:BTreeMap<(String,String,String),&Snapshot>=BTreeMap::new();
    for s in samples { if let Some(id)=s.session.as_ref().and_then(|m|m.id.as_ref()){let key=(s.provider.to_string(),s.account_id.clone(),id.clone());if latest.get(&key).is_none_or(|old|s.observed_at_ms>old.observed_at_ms){latest.insert(key,s);} } }
    let mut grouped:BTreeMap<String,Vec<&Snapshot>>=BTreeMap::new();
    for s in latest.values(){let project=s.session.as_ref().and_then(|m|m.project.clone()).unwrap_or_else(||"unassigned".into());grouped.entry(project).or_default().push(s);}
    grouped.into_iter().map(|(name,rows)|{let costs=rows.iter().filter_map(|s|s.metrics.as_ref().and_then(|m|m.estimated_cost_usd)).collect::<Vec<_>>();let cost=if costs.is_empty(){None}else{Some(costs.iter().sum::<f64>())};json!({"project":name,"session_count":rows.len(),"estimated_cost_usd":cost,"sessions_with_cost":costs.len(),"billed_cost_usd":Value::Null,"currency":"USD","cost_source":"reported_session_estimates","cost_coverage":if costs.is_empty(){"unavailable"}else if costs.len()<rows.len(){"partial"}else{"complete_for_observed_sessions"},"quota_attribution":"unavailable","observed_at_ms":rows.iter().map(|s|s.observed_at_ms).max()})}).collect()
}
pub fn agents(samples:&[Snapshot],now:u64)->Value {
    let mut nodes=Vec::new();
    for s in samples { let Some(meta)=&s.session else{continue};let Some(id)=&meta.id else{continue};let parent=meta.parent_id.as_ref().and_then(|pid|samples.iter().find(|p|p.provider==s.provider&&p.account_id==s.account_id&&p.source==s.source&&p.session.as_ref().and_then(|m|m.id.as_ref())==Some(pid)));
        let mut cycle=false;let mut cursor=parent;let mut visited=std::collections::HashSet::new();visited.insert(id.as_str());
        while let Some(p)=cursor { let Some(m)=&p.session else{break};let Some(pid)=m.id.as_deref() else{break};if !visited.insert(pid){cycle=true;break;}cursor=m.parent_id.as_ref().and_then(|next|samples.iter().find(|q|q.provider==s.provider&&q.account_id==s.account_id&&q.source==s.source&&q.session.as_ref().and_then(|m|m.id.as_ref())==Some(next)));if visited.len()>256{cycle=true;break;} }
        nodes.push(json!({"id":id,"provider":s.provider,"account_id":s.account_id,"source":s.source,"parent_id":if cycle{None}else{parent.and_then(|p|p.session.as_ref().and_then(|m|m.id.as_ref()))},"reported_parent_id":meta.parent_id,"relationship":if cycle{"invalid_cycle"}else if parent.is_some(){"reported_and_resolved"}else if meta.parent_id.is_some(){"unresolved"}else{"not_reported"},"model":s.model,"effort":s.metrics.as_ref().and_then(|m|m.effort.as_ref()),"project":meta.project,"reported_state":meta.state,"freshness":if s.is_recent(now){"recent"}else{"stale"}}));
    }
    json!({"availability":if nodes.is_empty(){"no_session_metadata"}else{"reported_metadata"},"inferred_relationships":false,"nodes":nodes})
}
pub fn resets(snapshots:&[Snapshot],now:u64)->Vec<Value>{let mut rows=Vec::new();for s in snapshots{for b in &s.buckets{if let Some(at)=b.resets_at_ms{rows.push(json!({"provider":s.provider,"account_id":s.account_id,"source":s.source,"status":s.status,"bucket_id":b.id,"label":b.label,"resets_at_ms":at,"reset_in_ms":at.saturating_sub(now),"awaiting_confirmation":at<=now}));}}}rows.sort_by_key(|r|r["resets_at_ms"].as_u64());rows}
pub fn cockpit(root:&Path,now:u64)->Result<Value,String>{
    let settings=config::load(root)?;let quotas=current(root,now)?;let all_sessions=sessions(root,now)?;let mut providers=Vec::new();let mut pressure=Vec::new();
    for p in PROVIDERS { let rows=quotas.iter().filter(|s|s.provider==p).collect::<Vec<_>>();let enabled=settings.providers.enabled(p);let status=if !enabled{"disabled"}else if rows.iter().any(|s|s.status==DataStatus::Live){"live"}else if rows.iter().any(|s|s.status==DataStatus::Error){"error"}else if rows.iter().any(|s|s.status==DataStatus::NeedsAuth){"needs_auth"}else if rows.iter().any(|s|s.status==DataStatus::Backoff){"backoff"}else if rows.iter().any(|s|s.status==DataStatus::Stale){"stale"}else{"unavailable"};
        let sources=rows.iter().map(|s|json!({"source":s.source,"account_id":s.account_id,"status":s.status,"observed_at_ms":s.observed_at_ms,"age_ms":if s.observed_at_ms==0{None}else{Some(now.saturating_sub(s.observed_at_ms))},"source_version":s.source_version,"bucket_count":s.buckets.len()})).collect::<Vec<_>>();
        let minimum=rows.iter().filter(|s|s.status==DataStatus::Live&&s.is_recent(now)).flat_map(|s|s.buckets.iter()).filter(|b|!b.awaiting_reset_confirmation(now)).filter_map(|b|b.remaining_fraction).reduce(f64::min);
        let state=match minimum{Some(n) if n<=settings.reserve_fraction=>"critical",Some(n) if n<=0.25=>"pressure",Some(_)=>"ready",None=>"unknown"};
        pressure.push(json!({"provider":p,"state":state,"lowest_reported_bucket_remaining":minimum,"aggregation":"most_constrained_independent_bucket"}));
        providers.push(json!({"id":p,"enabled":enabled,"status":status,"sources":sources,"capabilities":{"quota":rows.iter().any(|s|s.buckets.iter().any(|b|b.remaining_fraction.is_some())),"sessions":all_sessions.iter().any(|s|s.provider==p),"billing":false}}));
    }
    let state=if pressure.iter().any(|v|v["state"]=="critical"){"critical"}else if pressure.iter().any(|v|v["state"]=="pressure"){"pressure"}else if pressure.iter().any(|v|v["state"]=="ready"){"ready"}else{"unknown"};
    let visible=all_sessions.iter().take(500).cloned().collect::<Vec<_>>();
    Ok(json!({"schema_version":SCHEMA_VERSION,"captured_at_ms":now,"providers":providers,"quotas":quotas,"sessions":session_views(&visible,now),"session_count":all_sessions.len(),"sessions_truncated":all_sessions.len()>500,"projects":projects(&all_sessions),"projects_history_capped":all_sessions.len()>=5000,"agents":agents(&visible,now),"pulse":{"state":state,"capacity_percentage":Value::Null,"providers":pressure},"resets":resets(&quotas,now),"settings":settings}))
}
pub fn diagnostics(root:&Path,now:u64)->Value {
    let preferences=config::load(root);let current=current(root,now);let session_count=sessions(root,now).map(|s|s.len()).ok();
    let providers=PROVIDERS.iter().map(|p|{let rows=current.as_ref().ok().map(|s|s.iter().filter(|s|s.provider==*p).collect::<Vec<_>>()).unwrap_or_default();json!({"provider":p,"snapshot_count":rows.len(),"sources":rows.iter().map(|s|json!({"source":s.source,"status":s.status,"bucket_count":s.buckets.len(),"age_ms":if s.observed_at_ms==0{None}else{Some(now.saturating_sub(s.observed_at_ms))}})).collect::<Vec<_>>()})}).collect::<Vec<_>>();
    json!({"schema_version":1,"application":"nyrva","core_version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"captured_at_ms":now,"preferences_valid":preferences.is_ok(),"history_present":cli::database_path(root).exists(),"history_readable":current.is_ok(),"session_count":session_count,"providers":providers,"network_collection_triggered":false,"credentials_read":false,"redaction":"allowlist; excludes paths, session IDs, models, project names, payloads and credentials"})
}
