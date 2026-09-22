//! Conservative same-window estimates; never combines providers, accounts or quotas.
use crate::{DataStatus, Snapshot, MAX_TIMESTAMP_MS};
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct Forecast {
    pub fraction_per_hour: f64,
    pub budget_per_hour: f64,
    pub estimated_exhaustion_at_ms: Option<u64>,
    pub sample_count: usize,
    pub sample_span_ms: u64,
    pub estimated: bool,
}
pub fn forecast(samples: &[Snapshot], bucket_id: &str, now_ms: u64, reserve: f64) -> Option<Forecast> {
    if samples.len() < 3 || !reserve.is_finite() || !(0.0..=1.0).contains(&reserve) { return None; }
    let identity = samples.last()?;
    if samples.iter().any(|s| s.validate().is_err() || s.provider != identity.provider || s.account_id != identity.account_id || s.source != identity.source) { return None; }
    let mut ordered = samples.iter().collect::<Vec<_>>(); ordered.sort_by_key(|s| s.observed_at_ms);
    ordered.dedup_by_key(|s| s.observed_at_ms);
    if ordered.len() < 3 { return None; }
    let first = *ordered.first()?; let last = *ordered.last()?;
    if last.effective_status(now_ms) != DataStatus::Live || last.observed_at_ms > now_ms { return None; }
    let last_bucket = last.buckets.iter().find(|b| b.id == bucket_id)?;
    let reset = last_bucket.resets_at_ms.filter(|r| *r > now_ms)?;
    let span = last.observed_at_ms.checked_sub(first.observed_at_ms)?;
    if span < 300_000 { return None; }
    let mut previous = 1.0;
    for s in &ordered {
        if s.validate().is_err() || s.provider != last.provider || s.account_id != last.account_id || s.source != last.source || s.status != DataStatus::Live { return None; }
        let b = s.buckets.iter().find(|b| b.id == bucket_id)?;
        if b.resets_at_ms != Some(reset) || b.estimated || b.count.is_some() { return None; }
        let remaining = b.remaining_fraction?;
        if remaining > previous + 1e-9 { return None; }
        previous = remaining;
    }
    let initial = first.buckets.iter().find(|b| b.id == bucket_id)?.remaining_fraction?;
    let remaining = last_bucket.remaining_fraction?;
    let rate = (initial - remaining).max(0.0) / (span as f64 / 3_600_000.0);
    let budget = (remaining - reserve).max(0.0) / ((reset - now_ms) as f64 / 3_600_000.0);
    let exhaustion = if rate > 0.0 {
        let duration = remaining / rate * 3_600_000.0;
        if duration.is_finite() && duration <= MAX_TIMESTAMP_MS as f64 {
            now_ms.checked_add(duration.round() as u64).filter(|t| *t <= MAX_TIMESTAMP_MS)
        } else { None }
    } else { None };
    Some(Forecast { fraction_per_hour: rate, budget_per_hour: budget, estimated_exhaustion_at_ms: exhaustion,
        sample_count: ordered.len(), sample_span_ms: span, estimated: true })
}
