//! Native acceptance harness. Uses the production window opener, plugin and UI.
//! Test commands and synthetic storage are not part of the shipped binary.
#[path = "../src/observatory.rs"]
mod observatory;
use nyrva_core::{cli, config, Bucket, DataStatus, Provider, Snapshot};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::{atomic::{AtomicBool, Ordering}, mpsc}, time::{Duration, Instant}};
use tauri::{Manager, WebviewWindow};

mod telemetry {
    use super::PathBuf;
    pub(crate) fn data_root() -> Result<PathBuf, String> {
        std::env::var_os("NYRVA_EXPERIENCE_SMOKE_DIR").map(PathBuf::from)
            .ok_or_else(|| "isolated smoke directory is required".into())
    }
}
static STARTED: AtomicBool = AtomicBool::new(false);

fn until(mut condition: impl FnMut() -> bool) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !condition() {
        if Instant::now() >= deadline { return Err("native window lifecycle condition timed out".into()); }
        std::thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}

#[tauri::command]
async fn experience_smoke_lifecycle(window: WebviewWindow) -> Result<Value, String> {
    if window.label() != "dashboard" { return Err("wrong test window".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        let app = window.app_handle().clone();
        // generate_context! also creates the configured hidden notch. Preserve it.
        let mut before: Vec<_> = app.webview_windows().into_keys().collect();
        before.sort();
        if !before.iter().any(|name| name == "notch") { return Err("configured notch is missing".into()); }
        window.close().map_err(|e| e.to_string())?;
        until(|| !window.is_visible().unwrap_or(true))?;
        if app.get_webview_window("dashboard").is_none() { return Err("close destroyed the dashboard".into()); }
        let handle = app.clone();
        let (sender, receiver) = mpsc::channel();
        app.run_on_main_thread(move || {
            let result = (|| -> Result<(), String> {
                for _ in 0..32 { observatory::open(&handle).map_err(|e| e.to_string())?; }
                Ok(())
            })();
            let _ = sender.send(result);
        }).map_err(|e| e.to_string())?;
        receiver.recv_timeout(Duration::from_secs(3)).map_err(|_| "main-thread opening callback did not return")??;
        until(|| window.is_visible().unwrap_or(false))?;
        let mut after: Vec<_> = app.webview_windows().into_keys().collect();
        after.sort();
        if after != before { return Err(format!("opening changed the window set: {before:?} -> {after:?}")); }
        let dashboards = after.iter().filter(|name| name.as_str() == "dashboard").count();
        Ok(json!({"close_hides":true,"reopen_visible":true,"rapid_requests":32,"dashboard_count":dashboards,"notch_preserved":true,"event_callback_returned":true}))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn experience_smoke_finish(window: WebviewWindow, mut report: Value) -> Result<(), String> {
    if window.label() != "dashboard" { return Err("wrong test window".into()); }
    let root = telemetry::data_root()?;
    let preserved = std::fs::read(root.join("provider-owned.json")).ok().as_deref() == Some(b"SYNTHETIC PROVIDER SENTINEL".as_slice());
    let returned = root.join("opening-callback-returned").is_file();
    report["provider_file_preserved"] = json!(preserved);
    report["opening_callback_returned"] = json!(returned);
    report["creation_deferred"] = json!(returned);
    report["harness"] = json!("production opener on the event-loop thread; real Tauri/webview/SQLite; synthetic data");
    report["exclusions"] = json!(["physical tray click", "physical multi-monitor DPI", "packaged installer", "real provider authentication", "Everywhere"]);
    let ok = report["ok"] == true && preserved && returned;
    report["ok"] = json!(ok);
    std::fs::write(root.join("native-smoke.json"), serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    window.app_handle().exit(if ok { 0 } else { 1 });
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = telemetry::data_root().map_err(std::io::Error::other)?;
    std::fs::create_dir(&root)?;
    std::fs::write(root.join("provider-owned.json"), b"SYNTHETIC PROVIDER SENTINEL")?;
    let now = cli::now_ms();
    let mut observation = Snapshot::empty(Provider::Antigravity, "active", "statusline", now);
    observation.status = DataStatus::Live;
    observation.buckets.push(Bucket { id: "weekly".into(), label: "<img src=x onerror=alert(1)>".into(), remaining_fraction: Some(0.0), resets_at_ms: Some(now + 3_600_000), count: None, estimated: false, window_seconds: Some(604_800) });
    config::record(&root, &mut observation).map_err(std::io::Error::other)?;
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![experience_smoke_finish, experience_smoke_lifecycle])
        .on_page_load(|webview, payload| {
            if webview.label() == "dashboard" && payload.event() == tauri::webview::PageLoadEvent::Finished && !STARTED.swap(true, Ordering::SeqCst) {
                if let Err(error) = webview.eval(include_str!("../../tests/experience/native-smoke.js")) {
                    eprintln!("native test injection failed: {error}");
                    webview.app_handle().exit(1);
                }
            }
        })
        .setup(move |app| {
            observatory::install(app.handle())?;
            let handle = app.handle().clone();
            let opening_root = root.clone();
            // Dispatch from another thread so this cannot execute inline in setup.
            std::thread::spawn(move || {
                let dispatcher = handle.clone();
                let _ = dispatcher.run_on_main_thread(move || {
                    match observatory::open(&handle) {
                        Ok(()) => {
                            // A deterministic contract, not a timing benchmark: creation
                            // must be deferred until this event callback can return.
                            if handle.get_webview_window("dashboard").is_some() {
                                let _ = std::fs::write(opening_root.join("native-smoke.json"), b"{\"ok\":false,\"reason\":\"window creation ran inline on the event-loop callback\"}");
                                handle.exit(1);
                                return;
                            }
                            let _ = std::fs::write(opening_root.join("opening-callback-returned"), b"deferred");
                        }
                        Err(error) => { eprintln!("production opener failed: {error}"); handle.exit(1); }
                    }
                });
            });
            let handle = app.handle().clone();
            let root = root.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(75));
                if !root.join("native-smoke.json").exists() {
                    let _ = std::fs::write(root.join("native-smoke.json"), b"{\"ok\":false,\"reason\":\"native smoke timed out\"}");
                    handle.exit(2);
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())?;
    Ok(())
}
