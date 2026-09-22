use crate::{Provider, Snapshot};
use std::path::Path;
/// Contract baseline; the RED persistence suite guards its implementation.
pub struct History;
impl History {
    pub fn open(_path: &Path) -> Result<Self, String> { Err("history not initialized".into()) }
    pub fn open_readonly(_path: &Path) -> Result<Self, String> { Err("history not initialized".into()) }
    pub fn record(&self, _snapshot: &Snapshot) -> Result<bool, String> { Err("history not initialized".into()) }
    pub fn latest(&self, _now_ms: u64) -> Result<Vec<Snapshot>, String> { Ok(vec![]) }
    pub fn samples(&self, _provider: Provider, _account: &str, _since_ms: u64, _limit: usize) -> Result<Vec<Snapshot>, String> { Ok(vec![]) }
    pub fn prune(&self, _before_ms: u64) -> Result<usize, String> { Ok(0) }
}
