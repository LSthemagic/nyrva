//! Isolated, focusable Experience window. The notch's window policy is unchanged.
#[path = "observatory_data.rs"]
mod data;

use nyrva_core::{alerts, cli, config};
use serde_json::{json, Value};
use std::{path::Path, sync::{atomic::{AtomicU64, Ordering}, Mutex}, time::Duration};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

static OPERATIONS: Mutex<()> = Mutex::new(());
static REVISION: AtomicU64 = AtomicU64::new(0);

fn permitted_url(url: &tauri::Url) -> bool {
    let origin = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (["http", "https"].contains(&url.scheme()) && url.host_str() == Some("tauri.localhost"));
    origin && url.port().is_none() && url.username().is_empty() && url.password().is_none()
        && url.path() == "/dashboard.html" && url.query().is_none()
}

fn authorize(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != "dashboard" || !window.url().is_ok_and(|url| permitted_url(&url)) {
        return Err("Experience commands require the local dashboard window".into());
    }
    Ok(())
}

fn packet(root: &Path) -> Result<Value, String> {
    let mut value = data::read(root, cli::now_ms())?;
    value["revision"] = json!(REVISION.fetch_add(1, Ordering::SeqCst) + 1);
    Ok(value)
}

async fn execute<T, F>(window: WebviewWindow, operation: F) -> Result<T, String>
where T: Send + 'static, F: FnOnce(&Path) -> Result<T, String> + Send + 'static {
    authorize(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = OPERATIONS.lock().map_err(|_| "local observatory is unavailable")?;
        let root = crate::telemetry::data_root()?;
        operation(&root)
    }).await.map_err(|_| "local observatory operation failed".to_string())?
}

#[tauri::command]
async fn read_cockpit(window: WebviewWindow) -> Result<Value, String> {
    execute(window, packet).await
}
#[tauri::command]
async fn read_history(window: WebviewWindow, query: data::HistoryQuery) -> Result<Value, String> {
    execute(window, move |root| data::history(root, &query, cli::now_ms())).await
}
#[tauri::command]
async fn save_settings(window: WebviewWindow, settings: config::Settings) -> Result<Value, String> {
    execute(window, move |root| { data::save_settings(root, &settings)?; packet(root) }).await
}
#[tauri::command]
async fn export_settings(window: WebviewWindow) -> Result<config::Settings, String> {
    execute(window, config::load).await
}
#[tauri::command]
async fn import_settings(window: WebviewWindow, text: String) -> Result<Value, String> {
    execute(window, move |root| { data::import_settings(root, &text)?; packet(root) }).await
}
#[tauri::command]
async fn rollback_settings(window: WebviewWindow) -> Result<Value, String> {
    execute(window, |root| { config::rollback(root)?; packet(root) }).await
}
#[tauri::command]
async fn clear_history(window: WebviewWindow, confirmation: String) -> Result<Value, String> {
    execute(window, move |root| { data::clear_history(root, &confirmation)?; packet(root) }).await
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let plugin = tauri::plugin::Builder::<tauri::Wry>::new("observatory")
        .invoke_handler(tauri::generate_handler![read_cockpit, read_history, save_settings,
            export_settings, import_settings, rollback_settings, clear_history])
        .build();
    app.plugin(plugin)?;
    let app = app.clone();
    std::thread::spawn(move || loop {
        let result = (|| -> Result<(), String> {
            let _guard = OPERATIONS.lock().map_err(|_| "local observatory unavailable")?;
            let root = crate::telemetry::data_root()?;
            let refreshed = alerts::refresh(&root, cli::now_ms())?;
            if refreshed.get("new_alerts").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
                // Never interpolate source-provided metadata into the legacy notice surface.
                let _ = app.emit_to("notch", "notice", "Novos alertas no Nyrva Experience.");
            }
            if app.get_webview_window("dashboard").is_some_and(|w| w.is_visible().unwrap_or(false)) {
                let value = packet(&root)?;
                app.emit_to("dashboard", "observatory", &value).map_err(|_| "cannot update local dashboard")?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = app.emit_to("dashboard", "observatory-error", "local_update_unavailable");
        }
        std::thread::sleep(Duration::from_secs(5));
    });
    Ok(())
}

pub fn open(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("dashboard") {
        window.unminimize()?;
        window.show()?;
        return window.set_focus();
    }
    let window = WebviewWindowBuilder::new(app, "dashboard", WebviewUrl::App("dashboard.html".into()))
        .title("Nyrva — Experience")
        .inner_size(1120.0, 780.0)
        .min_inner_size(640.0, 520.0)
        .resizable(true)
        .center()
        .on_navigation(permitted_url)
        .build()?;
    let reusable = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = reusable.hide();
        }
    });
    window.set_focus()
}

#[cfg(test)]
mod tests {
    use super::permitted_url;
    #[test]
    fn only_the_packaged_dashboard_can_navigate_and_invoke() {
        for url in ["tauri://localhost/dashboard.html", "http://tauri.localhost/dashboard.html", "https://tauri.localhost/dashboard.html#main"] {
            assert!(permitted_url(&tauri::Url::parse(url).unwrap()), "{url}");
        }
        for url in ["https://example.com/dashboard.html", "https://tauri.localhost.evil.test/dashboard.html", "https://tauri.localhost/notch.html", "file:///dashboard.html", "http://localhost/dashboard.html", "http://tauri.localhost:8080/dashboard.html", "http://user@tauri.localhost/dashboard.html", "http://tauri.localhost/dashboard.html?redirect=x"] {
            assert!(!permitted_url(&tauri::Url::parse(url).unwrap()), "{url}");
        }
    }
}
