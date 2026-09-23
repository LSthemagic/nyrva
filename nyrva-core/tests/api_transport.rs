//! Real subprocess/socket regression: accept() flags differ between Windows and Unix.
use nyrva_core::cli;
use serde_json::Value;
use std::{
    fs,
    io::{ErrorKind, Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Server {
    child: Child,
    root: PathBuf,
    session: Value,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}
impl Server {
    fn start() -> Self {
        let root = std::env::temp_dir().join(format!(
            "nyrva-transport-{}-{}",
            std::process::id(),
            cli::now_ms()
        ));
        fs::create_dir(&root).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_nyrva-telemetry"))
            .env("NYRVA_DATA_DIR", &root)
            .args(["serve", "--port", "0", "--duration", "10"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut server = Self { child, root, session: Value::Null };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(bytes) = fs::read(server.root.join("telemetry/api-session.json")) {
                if let Ok(session) = serde_json::from_slice(&bytes) {
                    server.session = session;
                    return server;
                }
            }
            assert!(Instant::now() < deadline, "API session startup timeout");
            assert!(server.child.try_wait().unwrap().is_none(), "API exited during startup");
            thread::sleep(Duration::from_millis(10));
        }
    }
}

#[test]
fn api_waits_for_delayed_and_segmented_headers_without_premature_rejection() {
    let server = Server::start();
    let address = server.session["address"].as_str().unwrap();
    let token = server.session["token"].as_str().unwrap();
    let complete = format!(
        "GET /v1/status HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {token}\r\n\r\n"
    );
    // First delay the entire request, then split a request across real TCP writes.
    // A nonblocking accepted socket incorrectly responds before its deadline.
    for prefix in ["", "GET /v1/status HTTP/1.1\r\n"] {
        let mut connection = TcpStream::connect(address).unwrap();
        connection.set_nodelay(true).unwrap();
        connection.set_read_timeout(Some(Duration::from_millis(150))).unwrap();
        connection.write_all(prefix.as_bytes()).unwrap();
        let mut byte = [0u8; 1];
        let before_headers = connection.read(&mut byte);
        assert!(
            matches!(before_headers, Err(ref e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)),
            "API answered or closed before a complete request and before the 2s deadline: {before_headers:?}"
        );
        connection.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        connection.write_all(complete[prefix.len()..].as_bytes()).unwrap();
        let mut response = String::new();
        connection.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "delayed valid GET was rejected");
        let body = response.split_once("\r\n\r\n").unwrap().1;
        assert_eq!(serde_json::from_str::<Value>(body).unwrap()["schema_version"], 1);
        assert!(!response.contains(token));
    }
    assert!(!cli::database_path(&server.root).exists(), "API read created history");
}
