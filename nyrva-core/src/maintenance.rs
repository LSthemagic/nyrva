//! Explicit local migrations and a fail-closed Ed25519 artifact verification gate.
use crate::{
    cli, commands,
    everywhere::{hex, unhex, Flags},
    private_fs, storage, History,
};
use ring::{digest, signature};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::Duration,
};
use url::Url;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Trust {
    schema_version: u32,
    public_key: String,
    origin: String,
    minimum_version: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    version: String,
    target: String,
    url: String,
    size: u64,
    sha256: String,
}
fn https(text: &str) -> Result<Url, String> {
    let u = Url::parse(text).map_err(|_| "invalid signed update URL")?;
    if u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return Err("update URLs require HTTPS without credentials, query or fragment".into());
    }
    Ok(u)
}
fn validate(t: &Trust) -> Result<(), String> {
    if t.schema_version != 1 {
        return Err("unsupported update trust schema".into());
    }
    unhex(&t.public_key, 32)?;
    let u = https(&t.origin)?;
    if !u.path().ends_with('/') {
        return Err("trusted update prefix must end with a slash".into());
    }
    semver::Version::parse(&t.minimum_version).map_err(|_| "invalid minimum update version")?;
    Ok(())
}
pub(crate) fn update(
    args: &[String],
    root: &Path,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> Result<(), String> {
    let action = args.first().map(String::as_str).unwrap_or("status");
    let rest = if args.is_empty() { args } else { &args[1..] };
    let path = storage::state_dir(root).join("update-trust.json");
    if action == "configure" {
        let f = Flags::parse(rest, &[], &["--apply", "--json"])?;
        if !f.has("--apply") {
            return Err("update trust configuration requires --apply".into());
        }
        let raw = commands::bounded(input)?;
        if raw.len() > 8192 {
            return Err("update trust configuration exceeds 8 KiB".into());
        }
        let trust: Trust =
            serde_json::from_slice(&raw).map_err(|_| "invalid update trust configuration")?;
        validate(&trust)?;
        let _lock = private_fs::lock(&storage::state_dir(root).join("update.lock"))?;
        private_fs::write(
            &path,
            &serde_json::to_vec_pretty(&trust).map_err(|_| "cannot encode update trust")?,
        )?;
        return cli::write_json(
            out,
            &json!({"schema_version":1,"configured":true,"automatic_installation":false}),
        );
    }
    if action == "status" {
        Flags::parse(rest, &[], &["--json"])?;
        let valid = private_fs::read(&path, 8192)?
            .map(|b| {
                serde_json::from_slice::<Trust>(&b)
                    .map_err(|_| "invalid update trust")
                    .and_then(|t| validate(&t).map_err(|_| "invalid update trust"))
            })
            .transpose()?;
        return cli::write_json(
            out,
            &json!({"schema_version":1,"configured":valid.is_some(),"verification":"Ed25519 signed manifest plus SHA-256 artifact","automatic_installation":false,"network_requests":false}),
        );
    }
    if action != "verify" {
        return Err("update requires status, configure --apply or verify".into());
    }
    let f = Flags::parse(
        rest,
        &["--manifest", "--signature", "--artifact"],
        &["--json"],
    )?;
    let trust: Trust = serde_json::from_slice(
        &private_fs::read(&path, 8192)?.ok_or("no publisher trust configured; refusing update")?,
    )
    .map_err(|_| "invalid update trust")?;
    validate(&trust)?;
    let raw = private_fs::read(
        Path::new(f.get("--manifest").ok_or("missing --manifest")?),
        16384,
    )?
    .ok_or("signed manifest missing")?;
    let sig = private_fs::read(
        Path::new(f.get("--signature").ok_or("missing --signature")?),
        256,
    )?
    .ok_or("manifest signature missing")?;
    let sig = unhex(
        std::str::from_utf8(&sig)
            .map_err(|_| "signature must be hexadecimal")?
            .trim(),
        64,
    )?;
    signature::UnparsedPublicKey::new(&signature::ED25519, unhex(&trust.public_key, 32)?)
        .verify(&raw, &sig)
        .map_err(|_| "publisher signature verification failed")?;
    let m: Manifest = serde_json::from_slice(&raw).map_err(|_| "invalid signed update manifest")?;
    let version = semver::Version::parse(&m.version).map_err(|_| "invalid signed version")?;
    let minimum = semver::Version::parse(&trust.minimum_version)
        .map_err(|_| "invalid configured minimum version")?;
    let origin = https(&trust.origin)?;
    let url = https(&m.url)?;
    if m.schema_version != 1
        || !version.pre.is_empty()
        || version <= minimum
        || m.target != format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
        || m.size == 0
        || m.size > 512 * 1024 * 1024
        || url.origin() != origin.origin()
        || !url.path().starts_with(origin.path())
    {
        return Err("signed update has an untrusted origin, target, size or version".into());
    }
    let expected = unhex(&m.sha256, 32)?;
    let artifact = Path::new(f.get("--artifact").ok_or("missing --artifact")?);
    storage::check_path(artifact)?;
    let metadata = fs::symlink_metadata(artifact).map_err(|_| "update artifact missing")?;
    if !metadata.is_file() || metadata.len() != m.size {
        return Err("update artifact size or type does not match signed manifest".into());
    }
    let mut file = fs::File::open(artifact).map_err(|_| "cannot read update artifact")?;
    let mut context = digest::Context::new(&digest::SHA256);
    let mut buffer = [0u8; 65536];
    let mut total = 0u64;
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| "cannot verify update artifact")?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > m.size {
            return Err("update artifact changed while verifying".into());
        }
        context.update(&buffer[..n]);
    }
    let hash = context.finish();
    if total != m.size || hash.as_ref() != expected {
        return Err("update artifact does not match signed digest".into());
    }
    cli::write_json(
        out,
        &json!({"schema_version":1,"verified":true,"version":m.version,"target":m.target,"sha256":hex(hash.as_ref()),"installed":false,"automatic_installation":false,"next_action":"install the verified artifact explicitly; verification is not a publisher release approval"}),
    )
}
pub(crate) fn migrate(args: &[String], root: &Path, out: &mut dyn Write) -> Result<(), String> {
    let action = args
        .first()
        .filter(|s| !s.starts_with('-'))
        .map(String::as_str)
        .unwrap_or("apply");
    let rest = if args.first().is_some_and(|s| !s.starts_with('-')) {
        &args[1..]
    } else {
        args
    };
    let f = Flags::parse(rest, &[], &["--apply", "--json"])?;
    if !["status", "apply", "rollback"].contains(&action) {
        return Err("migrate requires status, --apply or rollback --apply".into());
    }
    let path = cli::database_path(root);
    storage::check_path(&path)?;
    if action == "status" {
        let version = if path.exists() {
            let db = rusqlite::Connection::open_with_flags(
                &path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .map_err(|_| "cannot inspect schema")?;
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .map_err(|_| "cannot inspect schema")?
        } else {
            0
        };
        return cli::write_json(
            out,
            &json!({"schema_version":1,"database_version":version,"supported_version":2,"changed":false}),
        );
    }
    if !f.has("--apply") {
        return Err("migration changes require --apply".into());
    }
    let _lock = private_fs::lock(&storage::state_dir(root).join("migration.lock"))?;
    if action == "apply" {
        History::open(&path)?;
        return cli::write_json(
            out,
            &json!({"schema_version":1,"database_version":2,"migration":"transactional"}),
        );
    }
    History::open_readonly(&path)?;
    let mut db =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|_| "cannot open Nyrva history")?;
    db.busy_timeout(Duration::from_secs(5))
        .map_err(|_| "cannot configure migration timeout")?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| "history is busy")?;
    let version: i64 = tx
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|_| "cannot inspect schema")?;
    if version == 2 {
        let collisions:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM observations GROUP BY provider,account_id,source,observed_at_ms HAVING COUNT(*)>1)",[],|r|r.get(0)).map_err(|_|"cannot check lossless rollback")?;
        if collisions {
            return Err(
                "rollback would merge independent sessions; original history preserved".into(),
            );
        }
        tx.execute_batch("ALTER TABLE observations RENAME TO observations_v2;
          DROP INDEX observations_time; DROP INDEX observations_session;
          CREATE TABLE observations(provider TEXT NOT NULL,account_id TEXT NOT NULL,source TEXT NOT NULL,observed_at_ms INTEGER NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(provider,account_id,source,observed_at_ms)) WITHOUT ROWID;
          INSERT INTO observations SELECT provider,account_id,source,observed_at_ms,payload FROM observations_v2;
          CREATE INDEX observations_time ON observations(observed_at_ms);
          DROP TABLE observations_v2; PRAGMA user_version=1;").map_err(|_|"lossless rollback failed; transaction preserved original data")?;
    } else if version != 1 {
        return Err("unsupported history version".into());
    }
    tx.commit()
        .map_err(|_| "cannot commit migration rollback")?;
    cli::write_json(
        out,
        &json!({"schema_version":1,"database_version":1,"lossless":true,"next_ingestion_migrates_to":2}),
    )
}
