//! Optional bounded loopback HTTP/SSE service. It never writes through an HTTP route.
use crate::{
    cli,
    everywhere::{hex, Flags},
    observatory, private_fs, storage,
};
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
const HEADER_LIMIT: usize = 8192;
const RESPONSE_LIMIT: usize = 2 * 1024 * 1024;
const CLIENT_LIMIT: usize = 8;
struct Session {
    path: PathBuf,
    token: String,
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(Some(bytes)) = private_fs::read(&self.path, 4096) {
            if serde_json::from_slice::<Value>(&bytes)
                .ok()
                .is_some_and(|v| v["token"].as_str() == Some(&self.token))
            {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}
struct Slot(Arc<AtomicUsize>);
impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
struct Bounded(Vec<u8>);
impl Write for Bounded {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(b.len()) > RESPONSE_LIMIT {
            return Err(std::io::Error::other("response exceeds local API limit"));
        }
        self.0.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encoded(v: &Value) -> Result<Vec<u8>, String> {
    let mut out = Bounded(Vec::new());
    serde_json::to_writer(&mut out, v).map_err(|_| "local response exceeds limits")?;
    Ok(out.0)
}
fn response(stream: &mut TcpStream, code: u16, body: &[u8]) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    write!(stream,"HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",body.len())?;
    stream.write_all(body)
}
struct Request {
    method: String,
    target: String,
    headers: BTreeMap<String, String>,
}
fn request(stream: &mut TcpStream) -> Result<Request, u16> {
    let until = Instant::now() + Duration::from_secs(2);
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 512];
    loop {
        if Instant::now() >= until {
            return Err(408);
        }
        let n = stream.read(&mut buffer).map_err(|_| 408u16)?;
        if n == 0 {
            return Err(400);
        }
        bytes.extend_from_slice(&buffer[..n]);
        if bytes.len() > HEADER_LIMIT {
            return Err(431);
        }
        if let Some(at) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
            if bytes.len() != at + 4 {
                return Err(400);
            }
            break;
        }
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| 400u16)?;
    let mut lines = text[..text.len() - 4].split("\r\n");
    let first = lines.next().ok_or(400u16)?;
    let parts: Vec<_> = first.split(' ').collect();
    if parts.len() != 3
        || parts[2] != "HTTP/1.1"
        || !parts[1].starts_with('/')
        || parts[1].len() > 1024
    {
        return Err(400);
    }
    let mut headers = BTreeMap::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(400u16)?;
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.bytes().any(|b| b < 32 && b != b'\t' || b == 127)
        {
            return Err(400);
        }
        if headers
            .insert(name.to_ascii_lowercase(), value.trim().to_string())
            .is_some()
        {
            return Err(400);
        }
    }
    Ok(Request {
        method: parts[0].into(),
        target: parts[1].into(),
        headers,
    })
}
fn authorize(request: &Request, address: &str, token: &str) -> Result<(), u16> {
    if request.headers.get("origin").is_some()
        || request
            .headers
            .get("sec-fetch-site")
            .is_some_and(|v| v != "none")
    {
        return Err(403);
    }
    if request.headers.get("host").map(String::as_str) != Some(address) {
        return Err(403);
    }
    let supplied = request
        .headers
        .get("authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !bool::from(supplied.as_bytes().ct_eq(token.as_bytes())) {
        return Err(401);
    }
    if request.method != "GET" {
        return Err(405);
    }
    if request.headers.contains_key("transfer-encoding")
        || request
            .headers
            .get("content-length")
            .is_some_and(|v| v != "0")
    {
        return Err(413);
    }
    Ok(())
}
fn view(root: &Path, target: &str) -> Result<Value, u16> {
    let now = cli::now_ms();
    let value = match target {
        "/v1/status" => {
            json!({"schema_version":1,"captured_at_ms":now,"providers":cli::read_current(root,now).map_err(|_|503u16)?})
        }
        "/v1/cockpit" => observatory::cockpit(root, now).map_err(|_| 503u16)?,
        "/v1/resets" => {
            json!({"schema_version":1,"resets":observatory::resets(&cli::read_current(root,now).map_err(|_|503u16)?,now)})
        }
        "/v1/sessions" => {
            json!({"schema_version":1,"sessions":observatory::session_views(&observatory::sessions(root,now).map_err(|_|503u16)?,now)})
        }
        "/v1/projects" => {
            json!({"schema_version":1,"projects":observatory::projects(&observatory::sessions(root,now).map_err(|_|503u16)?)})
        }
        "/v1/agents" => {
            json!({"schema_version":1,"agents":observatory::agents(&observatory::sessions(root,now).map_err(|_|503u16)?,now)})
        }
        "/v1/doctor" => observatory::diagnostics(root, now),
        _ if target.starts_with("/v1/history?") => {
            let mut query = BTreeMap::new();
            for (k, v) in
                url::form_urlencoded::parse(target.split_once('?').ok_or(400u16)?.1.as_bytes())
            {
                if !["provider", "account", "source", "limit"].contains(&k.as_ref())
                    || query.insert(k.into_owned(), v.into_owned()).is_some()
                {
                    return Err(400);
                }
            }
            let mut args = vec![
                "history".into(),
                query.remove("provider").ok_or(400u16)?,
                "--json".into(),
            ];
            let limit = query
                .remove("limit")
                .unwrap_or_else(|| "100".into())
                .parse::<usize>()
                .map_err(|_| 400u16)?;
            if !(1..=200).contains(&limit) {
                return Err(400);
            }
            args.extend(["--limit".into(), limit.to_string()]);
            for (key, flag) in [("account", "--account"), ("source", "--source")] {
                if let Some(value) = query.remove(key) {
                    args.extend([flag.into(), value]);
                }
            }
            let mut out = Bounded(Vec::new());
            cli::run(&args, root, &mut std::io::empty(), &mut out).map_err(|_| 400u16)?;
            serde_json::from_slice(&out.0).map_err(|_| 503u16)?
        }
        _ => return Err(404),
    };
    Ok(value)
}
fn client(mut stream: TcpStream, root: &Path, address: &str, token: &str, stop: &AtomicBool) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let request = match request(&mut stream).and_then(|r| authorize(&r, address, token).map(|()| r))
    {
        Ok(r) => r,
        Err(code) => {
            let _ = response(
                &mut stream,
                code,
                b"{\"schema_version\":1,\"error\":\"request_rejected\"}",
            );
            return;
        }
    };
    if request.target == "/v1/events" {
        if stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n").is_err(){return;}
        for id in 1..=60 {
            if stop.load(Ordering::Acquire) {
                break;
            }
            let (kind, value) = match view(root, "/v1/status") {
                Ok(v) => ("snapshot", v),
                Err(_) => (
                    "unavailable",
                    json!({"schema_version":1,"error":"local_cache_unavailable"}),
                ),
            };
            let Ok(payload) = encoded(&value) else {
                break;
            };
            if write!(stream, "id: {id}\nevent: {kind}\ndata: ")
                .and_then(|()| stream.write_all(&payload))
                .and_then(|()| stream.write_all(b"\n\n"))
                .is_err()
            {
                break;
            }
            for _ in 0..10 {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    } else {
        match view(root, &request.target).and_then(|v| encoded(&v).map_err(|_| 503u16)) {
            Ok(body) => {
                let _ = response(&mut stream, 200, &body);
            }
            Err(code) => {
                let _ = response(
                    &mut stream,
                    code,
                    b"{\"schema_version\":1,\"error\":\"local_view_unavailable\"}",
                );
            }
        }
    }
}
pub(crate) fn run(args: &[String], root: &Path, out: &mut dyn Write) -> Result<(), String> {
    let flags = Flags::parse(args, &["--port", "--duration"], &[])?;
    let port = flags.number("--port", 0, 0, 65535)? as u16;
    let duration = flags.number("--duration", 3600, 1, 86400)?;
    let dir = storage::state_dir(root);
    let _lock = private_fs::lock(&dir.join("api.lock"))?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .map_err(|_| "cannot bind local API listener")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "cannot configure local listener")?;
    let address = listener
        .local_addr()
        .map_err(|_| "cannot read local listener")?
        .to_string();
    let mut secret = [0u8; 32];
    SystemRandom::new()
        .fill(&mut secret)
        .map_err(|_| "OS random generator unavailable")?;
    let token = hex(&secret);
    let session = Session {
        path: dir.join("api-session.json"),
        token: token.clone(),
    };
    private_fs::write(&session.path,&serde_json::to_vec(&json!({"schema_version":1,"address":address,"token":token,"created_at_ms":cli::now_ms(),"expires_at_ms":cli::now_ms()+duration*1000})).map_err(|_|"cannot encode local API session")?)?;
    cli::write_json(
        out,
        &json!({"schema_version":1,"address":address,"authentication":"private_session_file","read_only":true,"duration_seconds":duration,"max_connections":CLIENT_LIMIT}),
    )?;
    out.flush()
        .map_err(|_| "cannot write local listener summary")?;
    let active = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let until = Instant::now() + Duration::from_secs(duration);
    let mut workers: Vec<thread::JoinHandle<()>> = Vec::new();
    while Instant::now() < until {
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() || active.load(Ordering::Acquire) >= CLIENT_LIMIT {
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                    let _ = response(
                        &mut stream,
                        503,
                        b"{\"schema_version\":1,\"error\":\"busy\"}",
                    );
                    continue;
                }
                active.fetch_add(1, Ordering::AcqRel);
                let slot = Slot(active.clone());
                let root = root.to_path_buf();
                let address = address.clone();
                let token = token.clone();
                let stop = stop.clone();
                if let Ok(worker) = thread::Builder::new()
                    .name("nyrva-local-read".into())
                    .spawn(move || {
                        let _slot = slot;
                        client(stream, &root, &address, &token, &stop);
                    })
                {
                    workers.push(worker);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(_) => {
                stop.store(true, Ordering::Release);
                return Err("local listener failed".into());
            }
        }
        let mut i = 0;
        while i < workers.len() {
            if workers[i].is_finished() {
                let _ = workers.swap_remove(i).join();
            } else {
                i += 1;
            }
        }
    }
    stop.store(true, Ordering::Release);
    for worker in workers {
        let _ = worker.join();
    }
    drop(session);
    Ok(())
}
