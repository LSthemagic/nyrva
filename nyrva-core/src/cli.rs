//! Headless commands shared by the desktop executable and telemetry helper.
//! Only `ingest` / `statusline` write data. Display commands never query providers.
use crate::{
    analytics, parse_legacy, parse_statusline, validate_account, History, Provider, Snapshot,
    MAX_PAYLOAD_BYTES, SCHEMA_VERSION,
};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn database_path(root: &Path) -> PathBuf {
    root.join("telemetry").join("history.sqlite3")
}
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

struct Options {
    command: String,
    provider: Option<Provider>,
    account: String,
    account_explicit: bool,
    json: bool,
    limit: usize,
    bucket: Option<String>,
    reserve: f64,
}
fn options(args: &[String]) -> Result<Options, String> {
    let command = args.first().map(String::as_str).unwrap_or("status");
    if ![
        "status",
        "resets",
        "history",
        "sessions",
        "forecast",
        "ingest",
        "statusline",
        "help",
        "--help",
    ]
    .contains(&command)
    {
        return Err("unknown telemetry command; use help".into());
    }
    let mut o = Options {
        command: command.into(),
        provider: None,
        account: "active".into(),
        account_explicit: false,
        json: false,
        limit: 200,
        bucket: None,
        reserve: 0.1,
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--json" if !o.json && command != "statusline" => o.json = true,
            "--account" => {
                o.account_explicit = true;
                i += 1;
                o.account = args.get(i).ok_or("missing account alias")?.clone();
                validate_account(&o.account)?;
            }
            "--limit" if command == "history" => {
                i += 1;
                o.limit = args
                    .get(i)
                    .ok_or("missing history limit")?
                    .parse()
                    .map_err(|_| "invalid history limit")?;
                if o.limit == 0 || o.limit > 5_000 {
                    return Err("history limit must be 1-5000".into());
                }
            }
            "--bucket" if command == "forecast" => {
                i += 1;
                o.bucket = Some(args.get(i).ok_or("missing bucket id")?.clone());
            }
            "--reserve" if command == "forecast" => {
                i += 1;
                o.reserve = args
                    .get(i)
                    .ok_or("missing reserve fraction")?
                    .parse::<f64>()
                    .map_err(|_| "invalid reserve fraction")?;
                if !o.reserve.is_finite() || !(0.0..=1.0).contains(&o.reserve) {
                    return Err("reserve must be between 0 and 1".into());
                }
            }
            name if !name.starts_with('-') && o.provider.is_none() => {
                o.provider = Some(name.parse()?)
            }
            _ => return Err("unknown or misplaced telemetry argument".into()),
        }
        i += 1;
    }
    if ["ingest", "statusline", "history", "forecast"].contains(&command) && o.provider.is_none() {
        return Err("this command requires a provider".into());
    }
    if command == "forecast" && o.bucket.is_none() {
        return Err("forecast requires --bucket ID".into());
    }
    Ok(o)
}

pub fn run(
    args: &[String],
    root: &Path,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> Result<(), String> {
    let o = options(args)?;
    let now = now_ms();
    if o.command == "help" || o.command == "--help" {
        return writeln!(out, "Nyrva telemetry\n  status [PROVIDER] [--json]\n  resets [PROVIDER] [--json]\n  sessions [PROVIDER] [--json]\n  history PROVIDER [--account ALIAS] [--limit 1..5000] [--json]\n  forecast PROVIDER --bucket ID [--account ALIAS] [--reserve 0.1] [--json]\n  ingest antigravity [--account ALIAS] [--json] < payload.json\n  statusline antigravity [--account ALIAS]\n\nRead-only display commands use local cached data; account defaults to active.")
            .map_err(|_| "cannot write command output".into());
    }
    if o.command == "statusline" || o.command == "ingest" {
        let mut bytes = Vec::new();
        input
            .take((MAX_PAYLOAD_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| "cannot read statusline input")?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err("statusline exceeds 256 KiB".into());
        }
        let raw = std::str::from_utf8(&bytes).map_err(|_| "statusline input is not UTF-8")?;
        let snapshot =
            parse_statusline(o.provider.ok_or("provider required")?, &o.account, raw, now)?;
        let db = History::open(&database_path(root))?;
        let inserted = db.record(&snapshot)?;
        db.prune(now.saturating_sub(90 * 24 * 60 * 60 * 1000))?;
        if o.command == "statusline" {
            return writeln!(out, "{}", format_snapshot(&snapshot, now))
                .map_err(|_| "cannot write statusline".into());
        }
        return write_json(
            out,
            &json!({"schema_version":SCHEMA_VERSION,"recorded":inserted}),
        );
    }
    if o.command == "history" || o.command == "forecast" {
        let samples = if database_path(root).exists() {
            History::open_readonly(&database_path(root))?.samples(
                o.provider.ok_or("provider required")?,
                &o.account,
                0,
                o.limit,
            )?
        } else {
            Vec::new()
        };
        if o.command == "history" {
            if o.json {
                return write_json(
                    out,
                    &json!({"schema_version":SCHEMA_VERSION,"samples":samples}),
                );
            }
            for s in samples {
                writeln!(out, "{} {}", s.observed_at_ms, format_snapshot(&s, now))
                    .map_err(|_| "cannot write history")?;
            }
            return Ok(());
        }
        let bucket = o.bucket.as_deref().ok_or("bucket required")?;
        let selected = samples
            .last()
            .map(|last| {
                let reset = last
                    .buckets
                    .iter()
                    .find(|b| b.id == bucket)
                    .and_then(|b| b.resets_at_ms);
                samples
                    .iter()
                    .filter(|s| {
                        s.source == last.source
                            && s.buckets
                                .iter()
                                .any(|b| b.id == bucket && b.resets_at_ms == reset)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let forecast = analytics::forecast(&selected, bucket, now, o.reserve);
        return write_json(
            out,
            &json!({"schema_version":SCHEMA_VERSION,"forecast":forecast}),
        );
    }
    let mut snapshots = read_current(root, now)?;
    snapshots.retain(|s| {
        o.provider.is_none_or(|p| p == s.provider)
            && (!o.account_explicit || s.account_id == o.account)
    });
    match o.command.as_str() {
        "status" => {
            if o.json {
                return write_json(
                    out,
                    &json!({"schema_version":SCHEMA_VERSION,"captured_at_ms":now,"providers":snapshots}),
                );
            }
            if snapshots.is_empty() {
                writeln!(
                    out,
                    "No local telemetry available; no usage value was inferred."
                )
                .map_err(|_| "cannot write status")?;
            }
            for s in snapshots {
                writeln!(out, "{}", format_snapshot(&s, now)).map_err(|_| "cannot write status")?;
            }
        }
        "resets" => {
            let mut resets = Vec::new();
            for s in snapshots {
                for b in &s.buckets {
                    if let Some(at) = b.resets_at_ms {
                        resets.push(json!({"provider":s.provider,"account_id":s.account_id,"source":s.source,"status":s.status,
                            "bucket_id":b.id,"resets_at_ms":at,"reset_in_ms":at.saturating_sub(now),"awaiting_confirmation":at<=now}));
                    }
                }
            }
            resets.sort_by_key(|r| r["resets_at_ms"].as_u64());
            if o.json {
                return write_json(
                    out,
                    &json!({"schema_version":SCHEMA_VERSION,"resets":resets}),
                );
            }
            for r in resets {
                writeln!(
                    out,
                    "{} [{}] {}: {}",
                    r["provider"].as_str().unwrap_or(""),
                    r["account_id"].as_str().unwrap_or(""),
                    r["bucket_id"].as_str().unwrap_or(""),
                    reset_label(r["resets_at_ms"].as_u64(), now)
                )
                .map_err(|_| "cannot write resets")?;
            }
        }
        "sessions" => {
            let sessions = snapshots.iter().filter(|s| s.session.is_some()).map(|s| json!({"provider":s.provider,"account_id":s.account_id,"status":s.status,"observed_at_ms":s.observed_at_ms,"model":s.model,"context":s.context,"session":s.session})).collect::<Vec<_>>();
            write_json(
                out,
                &json!({"schema_version":SCHEMA_VERSION,"sessions":sessions}),
            )?;
        }
        _ => return Err("unknown display command".into()),
    }
    Ok(())
}

pub fn read_current(root: &Path, now: u64) -> Result<Vec<Snapshot>, String> {
    let path = database_path(root);
    let mut snapshots = if path.exists() {
        History::open_readonly(&path)?.latest(now)?
    } else {
        vec![]
    };
    for (provider, name) in [
        (Provider::Claude, "usage.json"),
        (Provider::Codex, "codex.json"),
        (Provider::Cursor, "cursor.json"),
        (Provider::Antigravity, "antigravity.json"),
    ] {
        let file = match fs::File::open(root.join(name)) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(format!("cannot read {provider} local usage cache")),
        };
        let mut data = String::new();
        file.take((MAX_PAYLOAD_BYTES + 1) as u64)
            .read_to_string(&mut data)
            .map_err(|_| format!("invalid {provider} usage cache"))?;
        if data.len() > MAX_PAYLOAD_BYTES {
            return Err(format!("{provider} usage cache exceeds size limit"));
        }
        let value = serde_json::from_str(&data)
            .map_err(|_| format!("invalid {provider} usage cache JSON"))?;
        let mut s = parse_legacy(provider, "active", &value)?;
        s.status = s.effective_status(now);
        if let Some(existing) = snapshots.iter_mut().find(|x| {
            x.provider == provider && x.account_id == "active" && x.source == "legacy_adapter"
        }) {
            if s.observed_at_ms >= existing.observed_at_ms {
                *existing = s;
            }
        } else {
            snapshots.push(s);
        }
    }
    snapshots.sort_by(|a, b| {
        (a.provider.as_str(), &a.account_id, &a.source).cmp(&(
            b.provider.as_str(),
            &b.account_id,
            &b.source,
        ))
    });
    Ok(snapshots)
}
pub fn format_snapshot(s: &Snapshot, now: u64) -> String {
    let mut parts = vec![format!(
        "{} [{}] {:?}",
        s.provider,
        s.account_id,
        s.effective_status(now)
    )];
    if let Some(model) = &s.model {
        parts.push(model.clone());
    }
    for b in &s.buckets {
        let value = match (b.remaining_fraction, b.count) {
            (Some(f), _) => format!(
                "{}{:.0}% left",
                if b.estimated { "~" } else { "" },
                f * 100.0
            ),
            (_, Some(n)) => format!("count {n}; quota unknown"),
            _ => "unavailable".into(),
        };
        parts.push(format!(
            "{} {value} ({})",
            b.label,
            reset_label(b.resets_at_ms, now)
        ));
    }
    if s.buckets.is_empty() {
        parts.push("quota unavailable".into());
    }
    if let Some(pct) = s.context.as_ref().and_then(|c| c.used_percentage) {
        parts.push(format!("context {pct:.0}%"));
    }
    parts.join(" | ")
}
fn reset_label(reset: Option<u64>, now: u64) -> String {
    match reset {
        None => "reset unknown".into(),
        Some(at) if at <= now => "reset awaiting confirmation".into(),
        Some(at) => {
            let secs = (at - now) / 1000;
            format!(
                "reset in {}h {:02}m {:02}s",
                secs / 3600,
                (secs % 3600) / 60,
                secs % 60
            )
        }
    }
}
fn write_json(out: &mut dyn Write, value: &serde_json::Value) -> Result<(), String> {
    serde_json::to_writer(&mut *out, value).map_err(|_| "cannot write telemetry JSON")?;
    writeln!(out).map_err(|_| "cannot finish telemetry output".into())
}
