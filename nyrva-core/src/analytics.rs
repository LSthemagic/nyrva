use crate::Snapshot;
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
pub fn forecast(_samples: &[Snapshot], _bucket_id: &str, _now_ms: u64, _reserve: f64) -> Option<Forecast> { None }
