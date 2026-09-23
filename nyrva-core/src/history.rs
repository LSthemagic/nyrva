//! Versioned SQLite history, owned exclusively by Nyrva. No provider files are opened.
use crate::{validate_account, Provider, Snapshot, MAX_PAYLOAD_BYTES, MAX_TIMESTAMP_MS};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{fs, path::Path, time::Duration};
const DATABASE_VERSION: i64 = 2;
const MAX_ROWS: i64 = 50_000;
pub struct History { conn: Connection }

impl History {
    pub fn open(path: &Path) -> Result<Self, String> {
        prepare_file(path)?;
        let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).map_err(|_| "cannot open telemetry database".to_string())?;
        conn.busy_timeout(Duration::from_secs(2)).map_err(|_| "cannot configure database timeout")?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| "telemetry database is busy".to_string())?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|_| "cannot read telemetry schema".to_string())?;
        if !(0..=DATABASE_VERSION).contains(&version) { return Err("unsupported telemetry database version".into()); }
        if version == 0 {
            tx.execute_batch("CREATE TABLE observations (
                provider TEXT NOT NULL, account_id TEXT NOT NULL, source TEXT NOT NULL,
                observed_at_ms INTEGER NOT NULL, payload TEXT NOT NULL, session_key TEXT NOT NULL,
                PRIMARY KEY(provider,account_id,source,observed_at_ms,session_key)
            ) WITHOUT ROWID;
            CREATE INDEX observations_time ON observations(observed_at_ms);
            CREATE INDEX observations_session ON observations(provider,account_id,source,session_key,observed_at_ms DESC);
            PRAGMA user_version=2;").map_err(|_| "telemetry database migration failed".to_string())?;
        } else if version == 1 {
            // Transactional rebuild: a crash rolls back to the complete v1 table, not half a migration.
            tx.execute_batch("ALTER TABLE observations RENAME TO observations_v1;
            DROP INDEX observations_time;
            CREATE TABLE observations (
                provider TEXT NOT NULL, account_id TEXT NOT NULL, source TEXT NOT NULL,
                observed_at_ms INTEGER NOT NULL, payload TEXT NOT NULL, session_key TEXT NOT NULL,
                PRIMARY KEY(provider,account_id,source,observed_at_ms,session_key)
            ) WITHOUT ROWID;
            INSERT INTO observations(provider,account_id,source,observed_at_ms,payload,session_key)
              SELECT provider,account_id,source,observed_at_ms,payload,COALESCE(json_extract(payload,'$.session.id'),'') FROM observations_v1;
            CREATE INDEX observations_time ON observations(observed_at_ms);
            CREATE INDEX observations_session ON observations(provider,account_id,source,session_key,observed_at_ms DESC);
            DROP TABLE observations_v1;
            PRAGMA user_version=2;").map_err(|_| "telemetry v2 migration failed; original data preserved".to_string())?;
        }
        tx.commit().map_err(|_| "cannot commit telemetry migration".to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA secure_delete=ON;").map_err(|_| "cannot configure telemetry database".to_string())?;
        Ok(Self { conn })
    }
    pub fn open_readonly(path: &Path) -> Result<Self, String> {
        check_regular_file(path)?;
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|_| "cannot read telemetry database".to_string())?;
        conn.busy_timeout(Duration::from_secs(2)).map_err(|_| "cannot configure database timeout")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|_| "cannot read telemetry schema".to_string())?;
        if !(1..=DATABASE_VERSION).contains(&version) { return Err("unsupported telemetry database version".into()); }
        Ok(Self { conn })
    }
    pub fn record(&self, snapshot: &Snapshot) -> Result<bool, String> {
        snapshot.validate()?;
        let payload = serde_json::to_string(snapshot).map_err(|_| "invalid telemetry snapshot")?;
        if payload.len() > MAX_PAYLOAD_BYTES { return Err("telemetry snapshot exceeds storage limit".into()); }
        let session_key = snapshot.session.as_ref().and_then(|s| s.id.as_deref()).unwrap_or("");
        let tx = self.conn.unchecked_transaction().map_err(|_| "cannot start telemetry transaction")?;
        let inserted = tx.execute("INSERT OR IGNORE INTO observations(provider,account_id,source,observed_at_ms,payload,session_key) VALUES (?1,?2,?3,?4,?5,?6)",
            params![snapshot.provider.as_str(), snapshot.account_id, snapshot.source, snapshot.observed_at_ms, payload, session_key]).map_err(|_| "cannot write telemetry observation")? > 0;
        if inserted {
            tx.execute("DELETE FROM observations WHERE (provider,account_id,source,observed_at_ms,session_key) IN
              (SELECT provider,account_id,source,observed_at_ms,session_key FROM observations ORDER BY observed_at_ms DESC,provider,account_id,source,session_key LIMIT -1 OFFSET ?1)", [MAX_ROWS]).map_err(|_| "cannot enforce telemetry retention")?;
        }
        tx.commit().map_err(|_| "cannot commit telemetry observation")?;
        Ok(inserted)
    }
    /// One quota observation per provider/account/source. Same-time ties are deterministic.
    /// Session metadata has an independent view below; quota recovery must never backdate sessions.
    pub fn latest(&self, now_ms: u64) -> Result<Vec<Snapshot>, String> {
        let mut stmt = self.conn.prepare("SELECT payload FROM (
          SELECT payload, ROW_NUMBER() OVER (PARTITION BY provider,account_id,source ORDER BY observed_at_ms DESC,COALESCE(json_extract(payload,'$.session.id'),'') DESC) AS n FROM observations)
          WHERE n=1 ORDER BY json_extract(payload,'$.provider'),json_extract(payload,'$.account_id'),json_extract(payload,'$.source')").map_err(|_| "cannot query current telemetry")?;
        let rows = stmt.query_map([], |r| r.get::<_,String>(0)).map_err(|_| "cannot read current telemetry")?;
        let mut out = Vec::new();
        for row in rows {
            let mut s = decode(&row.map_err(|_| "cannot read telemetry row")?)?;
            if s.buckets.is_empty() && s.status != crate::DataStatus::Live {
                let previous: Option<String> = self.conn.query_row("SELECT payload FROM observations WHERE provider=?1 AND account_id=?2 AND source=?3 AND observed_at_ms<=?4 AND json_array_length(payload,'$.buckets')>0 ORDER BY observed_at_ms DESC,COALESCE(json_extract(payload,'$.session.id'),'') DESC LIMIT 1",
                    params![s.provider.as_str(),s.account_id,s.source,s.observed_at_ms], |r| r.get(0)).optional().map_err(|_| "cannot recover previous quota observation")?;
                if let Some(payload) = previous {
                    let status = match s.status { crate::DataStatus::NeedsAuth | crate::DataStatus::Backoff | crate::DataStatus::Error => s.status, _ => crate::DataStatus::Stale };
                    s = decode(&payload)?; s.status = status;
                }
            }
            s.status = s.effective_status(now_ms); out.push(s);
        }
        Ok(out)
    }
    /// Most recent metadata per session, including sessions with no published quota.
    /// The SQL works on v1 read-only databases too; reads never trigger a migration.
    pub fn sessions(&self, now_ms: u64) -> Result<Vec<Snapshot>, String> {
        let mut stmt = self.conn.prepare("SELECT payload FROM (
          SELECT payload,observed_at_ms,ROW_NUMBER() OVER (
            PARTITION BY provider,account_id,source,json_extract(payload,'$.session.id') ORDER BY observed_at_ms DESC
          ) AS n FROM observations WHERE json_type(payload,'$.session.id')='text' AND json_extract(payload,'$.session.id')<>'')
          WHERE n=1 ORDER BY observed_at_ms DESC LIMIT 5000").map_err(|_| "cannot query sessions")?;
        let rows = stmt.query_map([],|r| r.get::<_,String>(0)).map_err(|_| "cannot read sessions")?;
        let mut out = Vec::new();
        for row in rows { let mut s = decode(&row.map_err(|_| "cannot read session row")?)?; s.status = s.effective_status(now_ms); out.push(s); }
        Ok(out)
    }
    pub fn samples(&self, provider: Provider, account: &str, since_ms: u64, limit: usize) -> Result<Vec<Snapshot>, String> {
        validate_account(account)?;
        if limit == 0 || limit > 5000 || since_ms > MAX_TIMESTAMP_MS { return Err("invalid history query range".into()); }
        let mut stmt = self.conn.prepare("SELECT payload FROM observations WHERE provider=?1 AND account_id=?2 AND observed_at_ms>=?3 ORDER BY observed_at_ms DESC,source,COALESCE(json_extract(payload,'$.session.id'),'') DESC LIMIT ?4").map_err(|_| "cannot query telemetry history")?;
        let rows = stmt.query_map(params![provider.as_str(),account,since_ms,limit as i64],|r| r.get::<_,String>(0)).map_err(|_| "cannot read telemetry history")?;
        let mut out = Vec::new(); for row in rows { out.push(decode(&row.map_err(|_| "cannot read telemetry row")?)?); }
        out.reverse(); Ok(out)
    }
    pub fn prune(&self, before_ms: u64) -> Result<usize, String> {
        if before_ms > MAX_TIMESTAMP_MS { return Err("invalid retention timestamp".into()); }
        self.conn.execute("DELETE FROM observations WHERE observed_at_ms<?1",[before_ms]).map_err(|_| "cannot prune telemetry history".into())
    }
}
fn decode(payload: &str) -> Result<Snapshot, String> {
    if payload.len() > MAX_PAYLOAD_BYTES { return Err("stored telemetry exceeds size limit".into()); }
    let s: Snapshot = serde_json::from_str(payload).map_err(|_| "invalid stored telemetry")?;
    s.validate()?; Ok(s)
}
fn check_regular_file(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "telemetry database does not exist")?;
    if !meta.file_type().is_file() { return Err("telemetry database must be a regular file, not a link".into()); } Ok(())
}
fn prepare_file(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        if !parent.exists() {
            let mut builder = fs::DirBuilder::new(); builder.recursive(true);
            #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
            builder.create(parent).map_err(|_| "cannot create private telemetry directory")?;
        }
    }
    let mut options = fs::OpenOptions::new(); options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    match options.open(path) { Ok(_) => Ok(()), Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists => check_regular_file(path), Err(_) => Err("cannot create private telemetry database".into()) }
}
