//! Bounded, versioned SQLite history. Provider-owned files are never opened here.
use crate::{validate_account, Provider, Snapshot, MAX_PAYLOAD_BYTES, MAX_TIMESTAMP_MS};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::{fs, path::Path, time::Duration};

const DATABASE_VERSION: i64 = 1;
const MAX_ROWS: i64 = 50_000;
pub struct History {
    conn: Connection,
}

impl History {
    pub fn open(path: &Path) -> Result<Self, String> {
        prepare_file(path)?;
        let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|_| "cannot open telemetry database".to_string())?;
        conn.busy_timeout(Duration::from_secs(2))
            .map_err(|_| "cannot configure database timeout")?;
        // Serialize first-open migrations across multiple statusline processes.
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| "telemetry database is busy".to_string())?;
        let version: i64 = tx
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|_| "cannot read telemetry schema".to_string())?;
        if version != 0 && version != DATABASE_VERSION {
            return Err("unsupported telemetry database version".into());
        }
        if version == 0 {
            tx.execute_batch(
                "CREATE TABLE observations (
                provider TEXT NOT NULL, account_id TEXT NOT NULL, source TEXT NOT NULL,
                observed_at_ms INTEGER NOT NULL, payload TEXT NOT NULL,
                PRIMARY KEY(provider, account_id, source, observed_at_ms)
            ) WITHOUT ROWID;
            CREATE INDEX observations_time ON observations(observed_at_ms);
            PRAGMA user_version=1;",
            )
            .map_err(|_| "telemetry database migration failed".to_string())?;
        }
        tx.commit()
            .map_err(|_| "cannot commit telemetry migration".to_string())?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA secure_delete=ON;",
        )
        .map_err(|_| "cannot configure telemetry database".to_string())?;
        Ok(Self { conn })
    }

    pub fn open_readonly(path: &Path) -> Result<Self, String> {
        check_regular_file(path)?;
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "cannot read telemetry database".to_string())?;
        conn.busy_timeout(Duration::from_secs(2))
            .map_err(|_| "cannot configure database timeout")?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|_| "cannot read telemetry schema".to_string())?;
        if version != DATABASE_VERSION {
            return Err("unsupported telemetry database version".into());
        }
        Ok(Self { conn })
    }

    pub fn record(&self, snapshot: &Snapshot) -> Result<bool, String> {
        snapshot.validate()?;
        let payload = serde_json::to_string(snapshot).map_err(|_| "invalid telemetry snapshot")?;
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err("telemetry snapshot exceeds storage limit".into());
        }
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|_| "cannot start telemetry transaction")?;
        let inserted = tx
            .execute(
                "INSERT OR IGNORE INTO observations
            (provider,account_id,source,observed_at_ms,payload) VALUES (?1,?2,?3,?4,?5)",
                params![
                    snapshot.provider.as_str(),
                    snapshot.account_id,
                    snapshot.source,
                    snapshot.observed_at_ms,
                    payload
                ],
            )
            .map_err(|_| "cannot write telemetry observation")?
            > 0;
        if inserted {
            tx.execute(
                "DELETE FROM observations WHERE (provider,account_id,source,observed_at_ms) IN
                (SELECT provider,account_id,source,observed_at_ms FROM observations
                 ORDER BY observed_at_ms DESC LIMIT -1 OFFSET ?1)",
                [MAX_ROWS],
            )
            .map_err(|_| "cannot enforce telemetry retention")?;
        }
        tx.commit()
            .map_err(|_| "cannot commit telemetry observation")?;
        Ok(inserted)
    }

    /// One latest observation per provider/account/source, not a merged quota.
    pub fn latest(&self, now_ms: u64) -> Result<Vec<Snapshot>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT o.payload FROM observations o WHERE NOT EXISTS
            (SELECT 1 FROM observations n WHERE n.provider=o.provider AND n.account_id=o.account_id
             AND n.source=o.source AND n.observed_at_ms>o.observed_at_ms)
            ORDER BY o.provider,o.account_id,o.source",
            )
            .map_err(|_| "cannot query current telemetry")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|_| "cannot read current telemetry")?;
        let mut out = Vec::new();
        for row in rows {
            let mut s = decode(&row.map_err(|_| "cannot read telemetry row")?)?;
            if s.buckets.is_empty() && s.status != crate::DataStatus::Live {
                let previous: Option<String> = self.conn.query_row(
                    "SELECT payload FROM observations WHERE provider=?1 AND account_id=?2 AND source=?3
                     AND observed_at_ms<?4 AND json_array_length(payload,'$.buckets')>0
                     ORDER BY observed_at_ms DESC LIMIT 1",
                    params![s.provider.as_str(), s.account_id, s.source, s.observed_at_ms], |r| r.get(0)
                ).optional().map_err(|_| "cannot recover previous quota observation")?;
                if let Some(payload) = previous {
                    let status = match s.status {
                        crate::DataStatus::NeedsAuth
                        | crate::DataStatus::Backoff
                        | crate::DataStatus::Error => s.status,
                        _ => crate::DataStatus::Stale,
                    };
                    s = decode(&payload)?;
                    s.status = status;
                }
            }
            s.status = s.effective_status(now_ms);
            out.push(s);
        }
        Ok(out)
    }

    /// Returns the newest N observations in chronological order, retaining source identity.
    pub fn samples(
        &self,
        provider: Provider,
        account: &str,
        since_ms: u64,
        limit: usize,
    ) -> Result<Vec<Snapshot>, String> {
        validate_account(account)?;
        if limit == 0 || limit > 5_000 || since_ms > MAX_TIMESTAMP_MS {
            return Err("invalid history query range".into());
        }
        let mut stmt = self
            .conn
            .prepare(
                "SELECT payload FROM observations WHERE provider=?1 AND account_id=?2
            AND observed_at_ms>=?3 ORDER BY observed_at_ms DESC,source LIMIT ?4",
            )
            .map_err(|_| "cannot query telemetry history")?;
        let rows = stmt
            .query_map(
                params![provider.as_str(), account, since_ms, limit as i64],
                |r| r.get::<_, String>(0),
            )
            .map_err(|_| "cannot read telemetry history")?;
        let mut out = Vec::new();
        for row in rows {
            out.push(decode(&row.map_err(|_| "cannot read telemetry row")?)?);
        }
        out.reverse();
        Ok(out)
    }

    pub fn prune(&self, before_ms: u64) -> Result<usize, String> {
        if before_ms > MAX_TIMESTAMP_MS {
            return Err("invalid retention timestamp".into());
        }
        self.conn
            .execute(
                "DELETE FROM observations WHERE observed_at_ms<?1",
                [before_ms],
            )
            .map_err(|_| "cannot prune telemetry history".into())
    }
}

fn decode(payload: &str) -> Result<Snapshot, String> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err("stored telemetry exceeds size limit".into());
    }
    let s: Snapshot = serde_json::from_str(payload).map_err(|_| "invalid stored telemetry")?;
    s.validate()?;
    Ok(s)
}
fn check_regular_file(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "telemetry database does not exist")?;
    if !meta.file_type().is_file() {
        return Err("telemetry database must be a regular file, not a link".into());
    }
    Ok(())
}
fn prepare_file(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        if !parent.exists() {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder
                .create(parent)
                .map_err(|_| "cannot create private telemetry directory")?;
        }
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => check_regular_file(path),
        Err(_) => Err("cannot create private telemetry database".into()),
    }
}
