//! Black-box contracts for the actual Everywhere command surface.
use nyrva_core::cli;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "nyrva-more-{}-{}-{}",
            std::process::id(),
            cli::now_ms(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn call(root: &Path, args: &[&str], input: Value) -> Result<Value, String> {
    let mut out = Vec::new();
    cli::run(
        &args.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
        root,
        &mut input.to_string().as_bytes(),
        &mut out,
    )?;
    serde_json::from_slice(&out).map_err(|e| e.to_string())
}
fn exe() -> &'static str {
    env!("CARGO_BIN_EXE_nyrva-telemetry")
}
#[test]
fn redirected_top_is_once_readonly_and_plain() {
    let t = Temp::new();
    let r = Command::new(exe())
        .env("NYRVA_DATA_DIR", t.0.join("absent"))
        .args(["top"])
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(!r.stdout.contains(&27));
    assert!(!t.0.join("absent").exists());
    assert!(String::from_utf8_lossy(&r.stdout).contains("Nyrva"));
}
#[test]
fn explicit_root_is_shared_by_ingestion_and_reads() {
    let t = Temp::new();
    let root = t.0.join("data with spaces");
    let a = [
        "--data-dir",
        root.to_str().unwrap(),
        "ingest",
        "antigravity",
        "--json",
    ];
    call(
        &t.0,
        &a,
        json!({"quota":{"weekly":{"remaining_fraction":0.0}}}),
    )
    .unwrap();
    let r = call(
        &t.0,
        &["--data-dir", root.to_str().unwrap(), "status", "--json"],
        Value::Null,
    )
    .unwrap();
    assert_eq!(r["providers"][0]["buckets"][0]["remaining_fraction"], 0.0);
    assert!(!cli::database_path(&t.0).exists());
}
#[test]
fn repeated_install_is_idempotent_and_generated_command_uses_same_root() {
    let t = Temp::new();
    let home = t.0.join("home with spaces");
    let root = t.0.join("observations");
    let h = home.to_str().unwrap();
    let a = [
        "integrations",
        "install",
        "antigravity",
        "--home",
        h,
        "--executable",
        exe(),
        "--apply",
    ];
    call(&root, &a, Value::Null).unwrap();
    let path = home.join(".gemini/antigravity-cli/settings.json");
    let first = fs::read(&path).unwrap();
    call(&root, &a, Value::Null).unwrap();
    assert_eq!(fs::read(&path).unwrap(), first);
    let v: Value = serde_json::from_slice(&first).unwrap();
    let command = v["statusLine"]["command"].as_str().unwrap();
    #[cfg(unix)]
    let mut c = {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };
    #[cfg(windows)]
    let mut c = {
        let mut c = Command::new("cmd");
        c.args(["/d", "/s", "/c", command]);
        c
    };
    let mut child = c
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"quota":{"weekly":{"remaining_fraction":0.0}}}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("0%"));
    assert_eq!(
        call(&root, &["status", "--json"], Value::Null).unwrap()["providers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    call(
        &root,
        &[
            "integrations",
            "remove",
            "antigravity",
            "--home",
            h,
            "--apply",
        ],
        Value::Null,
    )
    .unwrap();
    assert!(serde_json::from_slice::<Value>(&fs::read(path).unwrap())
        .unwrap()
        .get("statusLine")
        .is_none());
}
#[test]
fn invalid_provider_json_is_not_rewritten() {
    let t = Temp::new();
    let home = t.0.join("home");
    let path = home.join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    for raw in [
        r#"{"theme":"a","theme":"b"}"#,
        r#"{"other":{"x":1,"x":2}}"#,
        "[]",
        "{broken",
    ] {
        fs::write(&path, raw).unwrap();
        assert!(call(
            &t.0,
            &[
                "integrations",
                "install",
                "claude",
                "--home",
                home.to_str().unwrap(),
                "--executable",
                exe(),
                "--apply"
            ],
            Value::Null
        )
        .is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), raw);
    }
}
#[cfg(unix)]
#[test]
fn integration_refuses_symlink_settings_and_does_not_touch_target() {
    use std::os::unix::fs::symlink;
    let t = Temp::new();
    let home = t.0.join("home");
    let path = home.join(".claude/settings.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let target = t.0.join("protected");
    fs::write(&target, "{}").unwrap();
    symlink(&target, &path).unwrap();
    assert!(call(
        &t.0,
        &[
            "integrations",
            "install",
            "claude",
            "--home",
            home.to_str().unwrap(),
            "--executable",
            exe(),
            "--apply"
        ],
        Value::Null
    )
    .is_err());
    assert_eq!(fs::read_to_string(target).unwrap(), "{}");
}
struct Server {
    child: std::process::Child,
    session: Value,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn server(root: &Path, seconds: &str) -> Server {
    let mut child = Command::new(exe())
        .env("NYRVA_DATA_DIR", root)
        .args(["serve", "--port", "0", "--duration", seconds])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let path = root.join("telemetry/api-session.json");
    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(session) = serde_json::from_slice(&bytes) {
                return Server { child, session };
            }
        }
        if child.try_wait().unwrap().is_some() {
            panic!("server terminated before publishing session");
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("session timeout");
}
fn request(s: &Server, target: &str, extra: &str) -> String {
    let address = s.session["address"].as_str().unwrap();
    let token = s.session["token"].as_str().unwrap();
    let mut c = TcpStream::connect(address).unwrap();
    c.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    c.write_all(format!("GET {target} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {token}\r\n{extra}\r\n").as_bytes()).unwrap();
    let mut text = String::new();
    c.read_to_string(&mut text).unwrap();
    text
}
#[test]
fn api_rejects_duplicate_headers_host_spoofing_bodies_and_unknown_routes() {
    let t = Temp::new();
    let s = server(&t.0, "10");
    for extra in [
        "Host: evil.example\r\n",
        "Authorization: Bearer duplicate\r\n",
        "Content-Length: 1\r\n",
        "Transfer-Encoding: chunked\r\n",
    ] {
        assert!(
            !request(&s, "/v1/status", extra).starts_with("HTTP/1.1 200"),
            "{extra}"
        );
    }
    assert!(request(&s, "/v1/status", "Origin: null\r\n").starts_with("HTTP/1.1 403"));
    assert!(request(&s, "/v1/not-real", "").starts_with("HTTP/1.1 404"));
    assert!(!cli::database_path(&t.0).exists());
}
#[test]
fn api_rotates_tokens_preserves_active_session_and_removes_on_normal_exit() {
    let t = Temp::new();
    let mut s = server(&t.0, "2");
    let first = s.session.clone();
    let second = Command::new(exe())
        .env("NYRVA_DATA_DIR", &t.0)
        .args(["serve", "--duration", "1"])
        .output()
        .unwrap();
    assert!(!second.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(t.0.join("telemetry/api-session.json")).unwrap())
            .unwrap(),
        first
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(t.0.join("telemetry/api-session.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o077,
            0
        );
    }
    assert!(s.child.wait().unwrap().success());
    assert!(!t.0.join("telemetry/api-session.json").exists());
    let next = server(&t.0, "2");
    assert_ne!(next.session["token"], first["token"]);
}
#[test]
fn api_stream_uses_versioned_sanitized_snapshots() {
    let t = Temp::new();
    let s = server(&t.0, "2");
    let r = request(&s, "/v1/events", "");
    assert!(r.starts_with("HTTP/1.1 200"));
    assert!(r.contains("text/event-stream"));
    let payload = r
        .lines()
        .find_map(|l| l.strip_prefix("data: "))
        .expect("SSE payload");
    assert_eq!(
        serde_json::from_str::<Value>(payload).unwrap()["schema_version"],
        1
    );
    assert!(!r.contains(s.session["token"].as_str().unwrap()));
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[test]
fn update_requires_trust_and_verifies_signature_bytes_origin_target_and_version() {
    use ring::{
        rand::SystemRandom,
        signature::{Ed25519KeyPair, KeyPair},
    };
    let t = Temp::new();
    let rng = SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let key = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    let artifact = t.0.join("nyrva.bin");
    fs::write(&artifact, b"synthetic installer not executable").unwrap();
    let manifest = t.0.join("manifest.json");
    let signature = t.0.join("manifest.sig");
    let bytes = fs::read(&artifact).unwrap();
    let mut m = json!({"schema_version":1,"version":"1.0.0","target":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),"url":"https://updates.example/nyrva/nyrva.bin","size":bytes.len(),"sha256":hex(ring::digest::digest(&ring::digest::SHA256,&bytes).as_ref())});
    let sign = |v: &Value| {
        let raw = serde_json::to_vec(v).unwrap();
        fs::write(&manifest, &raw).unwrap();
        fs::write(&signature, hex(key.sign(&raw).as_ref())).unwrap();
    };
    sign(&m);
    let args = [
        "update",
        "verify",
        "--manifest",
        manifest.to_str().unwrap(),
        "--signature",
        signature.to_str().unwrap(),
        "--artifact",
        artifact.to_str().unwrap(),
    ];
    assert!(call(&t.0, &args, Value::Null).is_err());
    let trust = json!({"schema_version":1,"public_key":hex(key.public_key().as_ref()),"origin":"https://updates.example/nyrva/","minimum_version":"0.3.0"});
    assert!(call(&t.0, &["update", "configure"], trust.clone()).is_err());
    call(&t.0, &["update", "configure", "--apply"], trust).unwrap();
    assert_eq!(call(&t.0, &args, Value::Null).unwrap()["verified"], true);
    fs::write(&artifact, b"tampered").unwrap();
    assert!(call(&t.0, &args, Value::Null).is_err());
    fs::write(&artifact, &bytes).unwrap();
    fs::write(&signature, "00".repeat(64)).unwrap();
    assert!(call(&t.0, &args, Value::Null).is_err());
    for (field, bad) in [
        ("url", "https://evil.example/nyrva.bin"),
        ("target", "not-this-platform"),
        ("version", "0.2.0"),
    ] {
        let old = m[field].clone();
        m[field] = json!(bad);
        sign(&m);
        assert!(call(&t.0, &args, Value::Null).is_err(), "{field}");
        m[field] = old;
    }
    assert_eq!(fs::read(artifact).unwrap(), bytes);
}
#[test]
fn migration_is_explicit_reversible_and_refuses_future_versions() {
    let t = Temp::new();
    assert!(call(&t.0, &["migrate"], Value::Null).is_err());
    call(&t.0, &["migrate", "--apply"], Value::Null).unwrap();
    let path = cli::database_path(&t.0);
    call(&t.0, &["migrate", "rollback", "--apply"], Value::Null).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(db);
    call(&t.0, &["migrate", "--apply"], Value::Null).unwrap();
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    db.execute_batch("PRAGMA user_version=99").unwrap();
    drop(db);
    assert!(call(&t.0, &["migrate", "--apply"], Value::Null).is_err());
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        99
    );
}

#[test]
fn interrupted_removal_cleans_only_the_receipt_when_previous_field_is_already_restored() {
    let t = Temp::new();
    let home = t.0.join("home");
    let target = home.join(".claude/settings.json");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, r#"{"statusLine":null,"theme":"preserve"}"#).unwrap();
    call(
        &t.0,
        &[
            "integrations",
            "install",
            "claude",
            "--home",
            home.to_str().unwrap(),
            "--executable",
            exe(),
            "--apply",
            "--replace",
        ],
        Value::Null,
    )
    .unwrap();
    let receipt = fs::read_dir(t.0.join("telemetry/integrations"))
        .unwrap()
        .map(|v| v.unwrap().path())
        .find(|p| p.extension().is_some_and(|s| s == "json"))
        .unwrap();
    let owned = fs::read(&receipt).unwrap();
    let remove = [
        "integrations",
        "remove",
        "claude",
        "--home",
        home.to_str().unwrap(),
        "--apply",
    ];
    call(&t.0, &remove, Value::Null).unwrap();
    // Reproduce interruption after provider restoration but before receipt cleanup.
    fs::write(&receipt, owned).unwrap();
    let restored = fs::read(&target).unwrap();
    call(&t.0, &remove, Value::Null)
        .expect("completed restoration must be recoverable without another provider write");
    assert!(!receipt.exists());
    assert_eq!(fs::read(target).unwrap(), restored);
}

#[test]
fn migration_rollback_refuses_session_collisions_without_losing_rows() {
    use nyrva_core::{parse_statusline, History, Provider};
    let t = Temp::new();
    let path = cli::database_path(&t.0);
    let db = History::open(&path).unwrap();
    let now = cli::now_ms();
    for id in ["one", "two"] {
        let s = parse_statusline(
            Provider::Claude,
            "active",
            &json!({"session_id":id,"model":{"id":"test-model"}}).to_string(),
            now,
        )
        .unwrap();
        db.record(&s).unwrap();
    }
    drop(db);
    assert!(call(&t.0, &["migrate", "rollback", "--apply"], Value::Null).is_err());
    let db = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM observations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}
