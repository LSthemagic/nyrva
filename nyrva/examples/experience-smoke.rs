//! Native WebKit/WebView2 acceptance harness. This example is not the shipped binary.
//! It imports the real plugin, HTML, capabilities and core; there is no mocked IPC.
#[path = "../src/observatory.rs"]
mod observatory;
use nyrva_core::{cli, config, Bucket, DataStatus, Provider, Snapshot};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::atomic::{AtomicBool, Ordering}, time::Duration};
use tauri::{Manager, WebviewWindow};

mod telemetry {
    use super::PathBuf;
    pub(crate) fn data_root() -> Result<PathBuf, String> {
        std::env::var_os("NYRVA_EXPERIENCE_SMOKE_DIR").map(PathBuf::from)
            .ok_or_else(|| "isolated smoke directory is required".into())
    }
}
static STARTED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn experience_smoke_finish(window: WebviewWindow, mut report: Value) -> Result<(), String> {
    if window.label() != "dashboard" { return Err("wrong test window".into()); }
    let root = telemetry::data_root()?;
    let preserved = std::fs::read(root.join("provider-owned.json")).ok().as_deref() == Some(b"SYNTHETIC PROVIDER SENTINEL".as_slice());
    report["provider_file_preserved"] = json!(preserved);
    report["harness"] = json!("real Tauri plugin and native webview; isolated synthetic data");
    report["exclusions"] = json!(["physical multi-monitor DPI", "packaged installer", "real provider authentication", "Everywhere"]);
    let ok = report["ok"] == true && preserved;
    std::fs::write(root.join("native-smoke.json"), serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    window.app_handle().exit(if ok { 0 } else { 1 });
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = telemetry::data_root().map_err(std::io::Error::other)?;
    // Refuse existing directories: this harness must never use real user data.
    std::fs::create_dir(&root)?;
    std::fs::write(root.join("provider-owned.json"), b"SYNTHETIC PROVIDER SENTINEL")?;
    let now = cli::now_ms();
    let mut observation = Snapshot::empty(Provider::Antigravity, "active", "statusline", now);
    observation.status = DataStatus::Live;
    observation.buckets.push(Bucket { id: "weekly".into(), label: "<img src=x onerror=alert(1)>".into(), remaining_fraction: Some(0.0), resets_at_ms: Some(now + 3_600_000), count: None, estimated: false, window_seconds: Some(604_800) });
    config::record(&root, &mut observation).map_err(std::io::Error::other)?;
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![experience_smoke_finish])
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
            observatory::open(app.handle())?;
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
