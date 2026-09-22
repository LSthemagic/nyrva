//! Antigravity (Google's IDE, Gemini quota) usage adapter.
//!
//! Data sources, in trust order:
//! 0. Opt-in Antigravity CLI statusline telemetry (active account, fresh).
//! 1. Antigravity's local `language_server` bridge.
//! 2. The last bridge reading, marked stale, when the IDE closes.
//! 3. Antigravity's own Google credential, borrowed read-only from the native credential store.
//! 4. A derived count from Antigravity transcripts when Google publishes no quota for the account.
//!
//! Linux parity is native: `/proc` is used to discover the language server and its listening
//! sockets, and Secret Service is accessed through the Rust `keyring` backend. No `ps`, `lsof`,
//! or provider-owned files are modified. Token values never reach logs, events, or the UI.

use crate::usage::{LimitWindow, UsageSnapshot};
use crate::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const POLL_SECS: u64 = 300;
const LOAD_CODE_ASSIST: &str = "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
const QUOTA_SUMMARY: &str = "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary";
const LS_SERVICE: &str = "/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary";
const CSRF_HEADER: &str = "x-codeium-csrf-token";

static REFRESH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn request_refresh() {
    REFRESH.store(true, std::sync::atomic::Ordering::Relaxed);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn state_root() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".gemini").join("antigravity"))
}

fn store_path() -> PathBuf {
    crate::config::config_path().with_file_name("antigravity.json")
}

pub fn load_persisted() -> UsageSnapshot {
    std::fs::read_to_string(store_path())
        .ok()
        .and_then(|t| serde_json::from_str::<UsageSnapshot>(&t).ok())
        .map(|mut s| {
            if !s.windows.is_empty() {
                s.status = "stale".into();
            }
            s
        })
        .unwrap_or_default()
}

fn persist(s: &UsageSnapshot) {
    if let Ok(t) = serde_json::to_string_pretty(s) {
        let _ = std::fs::write(store_path(), t);
    }
}

pub fn present() -> bool {
    crate::telemetry::antigravity_cli_present() || state_root().map(|p| p.is_dir()).unwrap_or(false) || read_credential_raw().is_some()
}

// ---------------- 1. Local bridge ----------------

#[derive(Clone, Debug, PartialEq)]
struct Endpoint {
    ports: Vec<u16>,
    csrf: String,
}

#[cfg(windows)]
fn run_hidden(program: &str, args: &[&str]) -> String {
    let mut cmd = std::process::Command::new(program);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000);
    cmd.output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

#[cfg(windows)]
fn discover() -> Option<Endpoint> {
    let table = run_hidden(
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-CimInstance Win32_Process -Filter \"Name LIKE '%language_server%'\" | ForEach-Object { \"$($_.ProcessId)`t$($_.CommandLine)\" }",
        ],
    );
    let line = table.lines().find(|l| l.contains("--csrf_token"))?;
    let (pid_s, cmdline) = line.split_once('\t')?;
    let pid: u32 = pid_s.trim().parse().ok()?;
    let csrf = flag_value(cmdline, "--csrf_token")?;
    let ports = listening_ports(pid);
    if ports.is_empty() {
        return None;
    }
    Some(Endpoint { ports, csrf })
}

#[cfg(target_os = "linux")]
fn discover() -> Option<Endpoint> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let pid: u32 = match entry.file_name().to_string_lossy().parse() {
            Ok(pid) => pid,
            Err(_) => continue,
        };
        let raw = match std::fs::read(format!("/proc/{pid}/cmdline")) {
            Ok(raw) if !raw.is_empty() => raw,
            _ => continue,
        };
        let args: Vec<String> = raw
            .split(|b| *b == 0)
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .collect();
        if !args.iter().any(|arg| arg.contains("language_server")) {
            continue;
        }
        let Some(csrf) = flag_value_args(&args, "--csrf_token") else {
            continue;
        };
        let ports = linux_listening_ports_from_proc(pid);
        if !ports.is_empty() {
            return Some(Endpoint { ports, csrf });
        }
    }
    None
}

#[cfg(not(any(windows, target_os = "linux")))]
fn discover() -> Option<Endpoint> {
    None
}

fn flag_value(line: &str, flag: &str) -> Option<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    let i = parts.iter().position(|part| *part == flag)?;
    parts.get(i + 1).map(|s| s.trim_matches('"').to_string())
}

#[cfg(target_os = "linux")]
fn flag_value_args(args: &[String], flag: &str) -> Option<String> {
    for (index, arg) in args.iter().enumerate() {
        if arg == flag {
            return args.get(index + 1).cloned().filter(|value| !value.is_empty());
        }
        if let Some(value) = arg.strip_prefix(flag).and_then(|rest| rest.strip_prefix('=')) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(windows)]
fn listening_ports(pid: u32) -> Vec<u16> {
    let out = run_hidden("netstat", &["-ano", "-p", "TCP"]);
    let pid_s = pid.to_string();
    let mut ports: Vec<u16> = out
        .lines()
        .filter(|line| line.contains("LISTENING"))
        .filter_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 5 || cols[4] != pid_s {
                return None;
            }
            cols[1].rsplit(':').next()?.parse().ok()
        })
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

#[cfg(target_os = "linux")]
fn process_socket_inodes(pid: u32) -> std::collections::HashSet<u64> {
    let mut inodes = std::collections::HashSet::new();
    let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
        return inodes;
    };
    for entry in entries.flatten() {
        let Ok(target) = std::fs::read_link(entry.path()) else {
            continue;
        };
        let text = target.to_string_lossy();
        let Some(inode) = text
            .strip_prefix("socket:[")
            .and_then(|value| value.strip_suffix(']'))
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };
        inodes.insert(inode);
    }
    inodes
}

#[cfg(target_os = "linux")]
fn parse_proc_net_listening_ports(
    text: &str,
    inodes: &std::collections::HashSet<u64>,
) -> Vec<u16> {
    let mut ports = Vec::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() <= 9 || cols[3] != "0A" {
            continue;
        }
        let Some(inode) = cols[9].parse::<u64>().ok() else {
            continue;
        };
        if !inodes.contains(&inode) {
            continue;
        }
        let Some(port_hex) = cols[1].rsplit(':').next() else {
            continue;
        };
        if let Ok(port) = u16::from_str_radix(port_hex, 16) {
            ports.push(port);
        }
    }
    ports
}

#[cfg(target_os = "linux")]
fn linux_listening_ports_from_proc(pid: u32) -> Vec<u16> {
    let inodes = process_socket_inodes(pid);
    if inodes.is_empty() {
        return Vec::new();
    }
    let mut ports = Vec::new();
    for name in ["tcp", "tcp6"] {
        if let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/net/{name}")) {
            ports.extend(parse_proc_net_listening_ports(&text, &inodes));
        }
    }
    ports.sort_unstable();
    ports.dedup();
    ports
}

fn local_agent() -> Option<ureq::Agent> {
    let tls = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
        .ok()?;
    Some(
        ureq::AgentBuilder::new()
            .tls_connector(Arc::new(tls))
            .timeout(Duration::from_secs(10))
            .build(),
    )
}

fn bridge_quota(ep: &Endpoint) -> Result<Vec<LimitWindow>, String> {
    let agent = local_agent().ok_or("TLS setup failed")?;
    let mut last = String::from("no port answered");
    for port in &ep.ports {
        let url = format!("https://127.0.0.1:{port}{LS_SERVICE}");
        match agent
            .post(&url)
            .set("Content-Type", "application/json")
            .set(CSRF_HEADER, &ep.csrf)
            .send_string(r#"{"forceRefresh":true}"#)
        {
            Ok(response) => match response.into_json::<serde_json::Value>() {
                Ok(value) => {
                    let windows = windows_from_bridge(&value);
                    if !windows.is_empty() {
                        return Ok(windows);
                    }
                    last = format!("port {port}: no recognisable groups");
                }
                Err(error) => last = format!("port {port}: {error}"),
            },
            Err(ureq::Error::Status(code, _)) => last = format!("port {port}: HTTP {code}"),
            Err(error) => last = format!("port {port}: {error}"),
        }
    }
    Err(last)
}

fn parse_iso(value: Option<&serde_json::Value>) -> Option<u64> {
    value
        .and_then(|value| value.as_str())
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|date| date.timestamp_millis().max(0) as u64)
}

pub fn windows_from_bridge(value: &serde_json::Value) -> Vec<LimitWindow> {
    let mut out = Vec::new();
    let Some(groups) = value.pointer("/response/groups").and_then(|groups| groups.as_array()) else {
        return out;
    };
    for group in groups {
        let group_name = group.get("displayName").and_then(|value| value.as_str());
        let Some(buckets) = group.get("buckets").and_then(|buckets| buckets.as_array()) else {
            continue;
        };
        for bucket in buckets {
            let Some(remaining) = bucket.get("remainingFraction").and_then(|value| value.as_f64()) else {
                continue;
            };
            if !(0.0..=1.0).contains(&remaining) {
                continue;
            }
            let bucket_name = bucket.get("displayName").and_then(|value| value.as_str());
            out.push(LimitWindow {
                id: bucket
                    .get("bucketId")
                    .and_then(|value| value.as_str())
                    .or(group_name)
                    .unwrap_or("quota")
                    .to_string(),
                label: group_name.or(bucket_name).unwrap_or("Usage").to_string(),
                used: (1.0 - remaining).clamp(0.0, 1.0),
                resets_at: parse_iso(bucket.get("resetTime")),
                ..Default::default()
            });
        }
    }
    out
}

// ---------------- 3. Credential store ----------------

struct Creds {
    access_token: String,
    expired: bool,
    auth_method: String,
}

#[cfg(windows)]
fn read_credential_raw() -> Option<Vec<u8>> {
    use windows::core::PCWSTR;
    use windows::Win32::Security::Credentials::{CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC};

    let target: Vec<u16> = "gemini:antigravity"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut pcred: *mut CREDENTIALW = std::ptr::null_mut();
    unsafe {
        if CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, 0, &mut pcred).is_err() || pcred.is_null() {
            return None;
        }
        let credential = &*pcred;
        let blob = if credential.CredentialBlobSize > 0 && !credential.CredentialBlob.is_null() {
            std::slice::from_raw_parts(
                credential.CredentialBlob,
                credential.CredentialBlobSize as usize,
            )
            .to_vec()
        } else {
            Vec::new()
        };
        CredFree(pcred as *const core::ffi::c_void);
        (!blob.is_empty()).then_some(blob)
    }
}

#[cfg(target_os = "linux")]
fn read_credential_raw() -> Option<Vec<u8>> {
    // Go keyring stores Antigravity with Secret Service attributes service=gemini,
    // username=antigravity. keyring-rs maps the same service/user pair to that item.
    let entry = keyring::Entry::new("gemini", "antigravity").ok()?;
    entry.get_secret().ok().filter(|secret| !secret.is_empty())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn read_credential_raw() -> Option<Vec<u8>> {
    None
}

fn decode_credential(raw: &[u8]) -> Option<Creds> {
    let mut text = String::from_utf8(raw.to_vec()).unwrap_or_else(|_| {
        let utf16: Vec<u16> = raw
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        String::from_utf16_lossy(&utf16)
    });
    text = text.trim_matches('\0').trim().to_string();
    if let Some(encoded) = text.strip_prefix("go-keyring-base64:") {
        text = String::from_utf8(b64_decode(encoded.trim())?).ok()?;
    }
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let access_token = value.pointer("/token/access_token")?.as_str()?.to_string();
    if access_token.is_empty() {
        return None;
    }
    let expiry = value
        .pointer("/token/expiry")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let expired = chrono::DateTime::parse_from_rfc3339(expiry)
        .map(|date| date.timestamp_millis().max(0) as u64 <= now_ms())
        .unwrap_or(false);
    let auth_method = value
        .get("auth_method")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    Some(Creds {
        access_token,
        expired,
        auth_method,
    })
}

pub(crate) fn b64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for byte in text.bytes() {
        let sextet = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        buf = (buf << 6) | sextet as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

fn read_credentials() -> Option<Creds> {
    decode_credential(&read_credential_raw()?)
}

fn load_tier(token: &str) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();
    match agent
        .post(LOAD_CODE_ASSIST)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .send_string(r#"{"metadata":{"pluginType":"GEMINI"}}"#)
    {
        Ok(response) => {
            let value: serde_json::Value = response.into_json().map_err(|error| error.to_string())?;
            let tier = value
                .get("currentTier")
                .or_else(|| {
                    value.get("allowedTiers").and_then(|tiers| tiers.as_array()).and_then(|tiers| {
                        tiers
                            .iter()
                            .find(|tier| tier.get("isDefault").and_then(|value| value.as_bool()) == Some(true))
                            .or(tiers.first())
                    })
                })
                .and_then(|tier| tier.get("name"))
                .and_then(|value| value.as_str())
                .unwrap_or("Gemini");
            Ok(tier.to_string())
        }
        Err(ureq::Error::Status(401, _)) | Err(ureq::Error::Status(403, _)) => Err("needsAuth".into()),
        Err(ureq::Error::Status(code, _)) => Err(format!("HTTP {code}")),
        Err(error) => Err(error.to_string()),
    }
}

fn direct_quota(token: &str) -> Option<Vec<LimitWindow>> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();
    let response = agent
        .post(QUOTA_SUMMARY)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .send_string("{}")
        .ok()?;
    let value: serde_json::Value = response.into_json().ok()?;

    let mut buckets = Vec::new();
    if let Some(groups) = value.get("quotaGroups").and_then(|groups| groups.as_array()) {
        for group in groups {
            if let Some(group_buckets) = group.get("buckets").and_then(|buckets| buckets.as_array()) {
                buckets.extend(group_buckets.iter().cloned());
            }
        }
    }
    if let Some(top_level) = value.get("buckets").and_then(|buckets| buckets.as_array()) {
        buckets.extend(top_level.iter().cloned());
    }

    let windows: Vec<LimitWindow> = buckets
        .iter()
        .filter_map(|bucket| {
            let limit = bucket.get("limit").and_then(|value| value.as_f64())?;
            let used = bucket.get("used").and_then(|value| value.as_f64())?;
            if limit <= 0.0 || used < 0.0 || used > limit * 1.5 {
                return None;
            }
            let label = bucket
                .get("displayName")
                .or_else(|| bucket.get("name"))
                .and_then(|value| value.as_str())
                .unwrap_or("Usage")
                .to_string();
            Some(LimitWindow {
                id: bucket
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or(&label)
                    .to_string(),
                label,
                used: (used / limit).clamp(0.0, 1.0),
                resets_at: parse_iso(bucket.get("resetTime")),
                ..Default::default()
            })
        })
        .collect();
    (!windows.is_empty()).then_some(windows)
}

// ---------------- 4. Transcript fallback ----------------

pub fn requests_today() -> (u64, Option<u64>) {
    use chrono::{Local, TimeZone};

    let Some(root) = state_root().map(|root| root.join("brain")) else {
        return (0, None);
    };
    let Ok(entries) = std::fs::read_dir(root) else {
        return (0, None);
    };
    let today = Local::now().date_naive();
    let mut count = 0u64;
    let mut latest = None;
    for entry in entries.flatten() {
        let path = entry
            .path()
            .join(".system_generated")
            .join("logs")
            .join("transcript.jsonl");
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in text.lines() {
            if !line.contains("\"MODEL\"") {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if value.get("source").and_then(|value| value.as_str()) != Some("MODEL") {
                continue;
            }
            let Some(timestamp) = value.get("created_at").and_then(|value| value.as_str()) else {
                continue;
            };
            let Ok(date) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
                continue;
            };
            let millis = date.timestamp_millis().max(0) as u64;
            latest = Some(latest.map_or(millis, |current: u64| current.max(millis)));
            if Local
                .timestamp_millis_opt(millis as i64)
                .single()
                .map(|local| local.date_naive() == today)
                .unwrap_or(false)
            {
                count += 1;
            }
        }
    }
    (count, latest)
}

// ---------------- Putting it together ----------------

struct Runtime {
    endpoint: Option<Endpoint>,
    ever_bridged: bool,
}

fn read_once(runtime: &mut Runtime, previous: &UsageSnapshot) -> UsageSnapshot {
    if let Some(cli) = crate::telemetry::antigravity_usage(true) { return cli; }

    let mut snapshot = UsageSnapshot::default();
    let mut bridge_error = String::new();
    let mut bridge_tried = false;

    if let Some(endpoint) = runtime.endpoint.clone() {
        bridge_tried = true;
        match bridge_quota(&endpoint) {
            Ok(windows) => {
                runtime.ever_bridged = true;
                snapshot.status = "ok".into();
                snapshot.windows = windows;
                snapshot.fetched_at = now_ms();
                snapshot.note = "via Antigravity".into();
                return snapshot;
            }
            Err(error) => {
                bridge_error = error;
                runtime.endpoint = None;
            }
        }
    }

    if let Some(endpoint) = discover() {
        bridge_tried = true;
        match bridge_quota(&endpoint) {
            Ok(windows) => {
                runtime.endpoint = Some(endpoint);
                runtime.ever_bridged = true;
                snapshot.status = "ok".into();
                snapshot.windows = windows;
                snapshot.fetched_at = now_ms();
                snapshot.note = "via Antigravity".into();
                return snapshot;
            }
            Err(error) => bridge_error = error,
        }
    }

    if bridge_tried && !bridge_error.is_empty() {
        crate::applog(&format!("antigravity: local bridge failed ({bridge_error})"));
    }

    if runtime.ever_bridged && !previous.windows.is_empty() {
        snapshot = previous.clone();
        snapshot.status = "stale".into();
        snapshot.note = "Antigravity is closed — last reading kept".into();
        return snapshot;
    }

    if let Some(cli) = crate::telemetry::antigravity_usage(false) { return cli; }

    let mut tier = None;
    match read_credentials() {
        Some(credentials) if !credentials.expired => match load_tier(&credentials.access_token) {
            Ok(current_tier) => {
                tier = Some(current_tier);
                if let Some(windows) = direct_quota(&credentials.access_token) {
                    snapshot.status = "ok".into();
                    snapshot.windows = windows;
                    snapshot.fetched_at = now_ms();
                    snapshot.note = format!("{} · via Google", tier.clone().unwrap_or_default());
                    return snapshot;
                }
            }
            Err(error) if error == "needsAuth" => {
                snapshot.status = "needsAuth".into();
                snapshot.note = "Antigravity's Google session was rejected — sign in again in Antigravity".into();
                return snapshot;
            }
            Err(error) => crate::applog(&format!("antigravity: loadCodeAssist {error}")),
        },
        Some(credentials) => {
            tier = Some(if credentials.auth_method == "consumer" {
                "Personal".into()
            } else {
                credentials.auth_method
            });
        }
        None => {}
    }

    let (count, latest) = requests_today();
    snapshot.status = "ok".into();
    snapshot.fetched_at = latest.unwrap_or_else(now_ms);
    snapshot.windows = vec![LimitWindow {
        id: "requests".into(),
        label: "Requests today · no limit published".into(),
        used: 0.0,
        resets_at: None,
        count: Some(count as i64),
        derived: true,
    }];
    snapshot.note = match tier {
        Some(tier) => format!("{tier} · Google publishes no quota for this account"),
        None => "Open Antigravity to read its quota".into(),
    };
    snapshot
}

fn broadcast(app: &AppHandle, snapshot: UsageSnapshot) {
    // A late IDE response must not overwrite the active, fresh CLI reading.
    let snapshot = crate::telemetry::antigravity_usage(true).unwrap_or(snapshot);
    let state = app.state::<AppState>();
    *state.antigravity.lock().unwrap() = snapshot.clone();
    persist(&snapshot);
    let _ = app.emit("antigravity", &snapshot);
}

fn sleep_interruptible(seconds: u64) {
    for _ in 0..seconds {
        if REFRESH.swap(false, std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        {
            let state = app.state::<AppState>();
            let snapshot = state.antigravity.lock().unwrap().clone();
            let _ = app.emit("antigravity", &snapshot);
        }
        if !present() {
            broadcast(
                &app,
                UsageSnapshot {
                    status: "absent".into(),
                    ..Default::default()
                },
            );
            loop {
                sleep_interruptible(600);
                if present() {
                    break;
                }
            }
        }

        let mut runtime = Runtime {
            endpoint: None,
            ever_bridged: false,
        };
        loop {
            let previous = {
                let state = app.state::<AppState>();
                let snapshot = state.antigravity.lock().unwrap().clone();
                snapshot
            };
            let snapshot = read_once(&mut runtime, &previous);
            broadcast(&app, snapshot);
            sleep_interruptible(POLL_SECS);
        }
    });
}

pub fn probe() -> String {
    let root = state_root().map(|path| path.display().to_string()).unwrap_or_default();
    let has_root = state_root().map(|path| path.is_dir()).unwrap_or(false);
    let credential = read_credentials();
    let endpoint = discover();
    format!(
        "Antigravity: state dir {} ({}) | credential store gemini:antigravity {} | language_server {}",
        root,
        if has_root { "present" } else { "missing" },
        match credential {
            Some(credential) => format!(
                "found ({}, {})",
                credential.auth_method,
                if credential.expired { "expired" } else { "valid" }
            ),
            None => "not found".into(),
        },
        match endpoint {
            Some(endpoint) => format!("running, ports {:?}", endpoint.ports),
            None => "not running".into(),
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_remaining_fraction_becomes_used_fraction() {
        let value = serde_json::json!({
            "response": {
                "groups": [{
                    "displayName": "Gemini",
                    "buckets": [{
                        "bucketId": "weekly",
                        "remainingFraction": 0.75,
                        "resetTime": "2030-01-01T00:00:00Z"
                    }]
                }]
            }
        });
        let windows = windows_from_bridge(&value);
        assert_eq!(windows.len(), 1);
        assert!((windows[0].used - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn credential_decode_keeps_secret_internal() {
        let raw = br#"{"auth_method":"consumer","token":{"access_token":"secret-value","expiry":"2030-01-01T00:00:00Z"}}"#;
        let credential = decode_credential(raw).expect("valid Antigravity credential");
        assert_eq!(credential.access_token, "secret-value");
        assert_eq!(credential.auth_method, "consumer");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_flag_parser_accepts_split_and_equals_forms() {
        let split = vec!["language_server".into(), "--csrf_token".into(), "abc".into()];
        let equals = vec!["language_server".into(), "--csrf_token=xyz".into()];
        assert_eq!(flag_value_args(&split, "--csrf_token").as_deref(), Some("abc"));
        assert_eq!(flag_value_args(&equals, "--csrf_token").as_deref(), Some("xyz"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn proc_net_parser_filters_listening_sockets_by_inode() {
        let text = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 0100007F:ABCD 00000000:0000 0A 00000000:00000000 00:00000000 00000000 1000 0 4242 1\n   1: 0100007F:1234 00000000:0000 01 00000000:00000000 00:00000000 00000000 1000 0 4242 1\n   2: 0100007F:2345 00000000:0000 0A 00000000:00000000 00:00000000 00000000 1000 0 9999 1\n";
        let inodes = std::collections::HashSet::from([4242]);
        assert_eq!(parse_proc_net_listening_ports(text, &inodes), vec![0xABCD]);
    }
}
