use crate::{
    model::clean_text, validate_account, Bucket, ContextUsage, DataStatus, Provider,
    SessionMetadata, Snapshot, MAX_BUCKETS, MAX_PAYLOAD_BYTES, MAX_TIMESTAMP_MS,
};
use serde_json::Value;

fn fraction(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64)
        .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
}
fn text(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .map(clean_text)
        .filter(|s| !s.is_empty())
}
fn timestamp(v: Option<&Value>) -> Option<u64> {
    let date = chrono::DateTime::parse_from_rfc3339(v?.as_str()?).ok()?;
    let ms = u64::try_from(date.timestamp_millis()).ok()?;
    (ms <= MAX_TIMESTAMP_MS).then_some(ms)
}
fn reset(v: &Value, now_ms: u64) -> Option<u64> {
    // An explicitly provided but invalid absolute timestamp is not silently replaced.
    if let Some(absolute) = v.get("reset_time").filter(|v| !v.is_null()) {
        return timestamp(Some(absolute));
    }
    let delta = v.get("reset_in_seconds")?.as_u64()?.checked_mul(1000)?;
    now_ms
        .checked_add(delta)
        .filter(|ms| *ms <= MAX_TIMESTAMP_MS)
}
fn context(v: &Value) -> Option<ContextUsage> {
    let c = v.get("context_window")?.as_object()?;
    let used_percentage = c
        .get("used_percentage")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && (0.0..=100.0).contains(v));
    let size = c
        .get("context_window_size")
        .and_then(Value::as_u64)
        .filter(|v| *v > 0 && *v <= 9_007_199_254_740_991);
    let current = c.get("current_usage");
    let input_tokens = current
        .and_then(|c| c.get("input_tokens"))
        .and_then(Value::as_u64);
    let output_tokens = current
        .and_then(|c| c.get("output_tokens"))
        .and_then(Value::as_u64);
    if used_percentage.is_none()
        && size.is_none()
        && input_tokens.is_none()
        && output_tokens.is_none()
    {
        None
    } else {
        Some(ContextUsage {
            used_percentage,
            size,
            input_tokens,
            output_tokens,
        })
    }
}
fn session(v: &Value) -> Option<SessionMetadata> {
    let id = text(v.get("session_id"));
    let parent_id = text(v.get("parent_session_id"));
    let project = v
        .pointer("/workspace/project_dir")
        .or_else(|| v.pointer("/workspace/current_dir"))
        .and_then(Value::as_str)
        .and_then(|s| s.rsplit(['/', '\\']).find(|p| !p.is_empty()))
        .map(clean_text);
    let branch = text(v.pointer("/vcs/branch"));
    let state = v
        .get("agent_state")
        .and_then(Value::as_str)
        .filter(|s| {
            [
                "idle",
                "thinking",
                "working",
                "tool_use",
                "initializing",
                "attention",
                "done",
            ]
            .contains(s)
        })
        .map(str::to_owned);
    if id.is_none() && project.is_none() && branch.is_none() && state.is_none() {
        None
    } else {
        Some(SessionMetadata {
            id,
            parent_id,
            project,
            branch,
            state,
        })
    }
}

/// Parse only documented Antigravity statusline metadata. Unknown fields are discarded.
/// This function never reads files, credentials, processes or the network.
pub fn parse_statusline(
    provider: Provider,
    account: &str,
    input: &str,
    now_ms: u64,
) -> Result<Snapshot, String> {
    validate_account(account)?;
    if provider != Provider::Antigravity {
        return Err("statusline ingestion is not implemented for this provider".into());
    }
    if input.len() > MAX_PAYLOAD_BYTES {
        return Err("statusline exceeds 256 KiB".into());
    }
    if now_ms > MAX_TIMESTAMP_MS {
        return Err("invalid observation time".into());
    }
    let v: Value =
        serde_json::from_str(input).map_err(|_| "invalid statusline JSON".to_string())?;
    if !v.is_object() {
        return Err("statusline must be a JSON object".into());
    }
    let mut out = Snapshot::empty(provider, account, "statusline", now_ms);
    out.model = text(
        v.pointer("/model/id")
            .or_else(|| v.pointer("/model/display_name")),
    );
    out.source_version = text(v.get("version"));
    out.context = context(&v);
    out.session = session(&v);
    if let Some(quota) = v.get("quota").and_then(Value::as_object) {
        if quota.len() > MAX_BUCKETS {
            return Err("too many quota buckets".into());
        }
        for (id, value) in quota {
            let Some(remaining) = fraction(value.get("remaining_fraction")) else {
                out.dropped_fields += 1;
                continue;
            };
            // Do not normalize identifiers into collisions. Preserve safe unknown future IDs.
            if id.is_empty()
                || id.len() > 512
                || id.chars().any(|c| {
                    c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                })
            {
                out.dropped_fields += 1;
                continue;
            }
            out.buckets.push(Bucket {
                id: id.clone(),
                label: clean_text(&id.replace(['_', '-'], " ")),
                remaining_fraction: Some(remaining),
                resets_at_ms: reset(value, now_ms),
                count: None,
                estimated: false,
                window_seconds: None,
            });
        }
    }
    out.buckets.sort_by(|a, b| a.id.cmp(&b.id));
    if !out.buckets.is_empty() {
        out.status = DataStatus::Live;
    }
    out.validate()?;
    Ok(out)
}

/// Adapter for the existing Nyrva UsageSnapshot JSON; never persists its free-text note.
pub fn parse_legacy(provider: Provider, account: &str, value: &Value) -> Result<Snapshot, String> {
    validate_account(account)?;
    if !value.is_object() {
        return Err("legacy snapshot must be an object".into());
    }
    let observed = value.get("fetched_at").and_then(Value::as_u64).unwrap_or(0);
    let mut out = Snapshot::empty(provider, account, "legacy_adapter", observed);
    out.status = match value.get("status").and_then(Value::as_str) {
        Some("ok") => DataStatus::Live,
        Some("stale") => DataStatus::Stale,
        Some("needsAuth") => DataStatus::NeedsAuth,
        Some("backoff") => DataStatus::Backoff,
        Some("error") => DataStatus::Error,
        _ => DataStatus::Unavailable,
    };
    if let Some(windows) = value.get("windows").and_then(Value::as_array) {
        if windows.len() > MAX_BUCKETS {
            return Err("too many quota buckets".into());
        }
        let mut ids = std::collections::HashSet::new();
        for w in windows {
            let Some(id) = w
                .get("id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_control))
            else {
                out.dropped_fields += 1;
                continue;
            };
            if !ids.insert(id) {
                out.dropped_fields += 1;
                continue;
            }
            let count = w.get("count").and_then(Value::as_u64);
            if w.get("count").is_some_and(|v| !v.is_null()) && count.is_none() {
                out.dropped_fields += 1;
                continue;
            }
            let remaining_fraction = if count.is_some() {
                None
            } else {
                fraction(w.get("used")).map(|f| 1.0 - f)
            };
            if count.is_none() && remaining_fraction.is_none() {
                out.dropped_fields += 1;
                continue;
            }
            out.buckets.push(Bucket {
                id: id.into(),
                label: text(w.get("label")).unwrap_or_else(|| clean_text(id)),
                remaining_fraction,
                resets_at_ms: w
                    .get("resets_at")
                    .and_then(Value::as_u64)
                    .filter(|n| *n <= MAX_TIMESTAMP_MS),
                count,
                estimated: w.get("derived").and_then(Value::as_bool).unwrap_or(false),
                window_seconds: None,
            });
        }
    }
    if out.buckets.is_empty() && out.status == DataStatus::Live {
        out.status = DataStatus::Unavailable;
    }
    if out.observed_at_ms == 0 && out.status == DataStatus::Live {
        out.status = DataStatus::Stale;
    }
    out.validate()?;
    Ok(out)
}
