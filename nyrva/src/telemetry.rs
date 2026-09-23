//! Desktop boundary for the headless telemetry core. No provider credentials are read here.
use crate::usage::{LimitWindow, UsageSnapshot};
use nyrva_core::{cli, config, DataStatus, History, Provider, Snapshot};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub(crate) fn data_root() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("NYRVA_DATA_DIR").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    dirs::config_dir()
        .map(|p| p.join("nyrva"))
        .ok_or_else(|| "Nyrva configuration directory is unavailable".into())
}

/// Called before constructing Tauri: commands work with no display or running GUI.
pub fn run_cli(args: &[String]) -> Option<i32> {
    let command = args.first()?.as_str();
    if ![
        "status",
        "resets",
        "history",
        "sessions",
        "forecast",
        "ingest",
        "statusline",
        "top",
        "doctor",
        "cockpit",
        "projects",
        "agents",
        "settings",
        "privacy",
        "alerts",
        "export",
        "integrations",
        "serve",
        "update",
        "migrate",
        "--data-dir",
        "help",
        "--help",
    ]
    .contains(&command)
    {
        return None;
    }
    let result = data_root().and_then(|root| {
        cli::run(
            args,
            &root,
            &mut std::io::stdin(),
            &mut std::io::stdout(),
        )
    });
    match result {
        Ok(()) => Some(0),
        Err(error) => {
            eprintln!("Nyrva telemetry: {error}");
            Some(1)
        }
    }
}

fn cli_snapshot() -> Option<Snapshot> {
    let root = data_root().ok()?;
    let db = History::open_readonly(&cli::database_path(&root)).ok()?;
    db.latest(cli::now_ms()).ok()?.into_iter().find(|s| {
        s.provider == Provider::Antigravity && s.source == "statusline" && s.account_id == "active"
    })
}

pub fn antigravity_cli_present() -> bool {
    dirs::home_dir().is_some_and(|h| h.join(".gemini/antigravity-cli").is_dir())
        || cli_snapshot().is_some()
}

/// The current notch is a single-account surface. Other aliases remain in history/CLI;
/// never silently display a different account just because its quota is higher/newer.
pub fn antigravity_usage(fresh_only: bool) -> Option<UsageSnapshot> {
    from_snapshot(&cli_snapshot()?, fresh_only, cli::now_ms())
}
fn from_snapshot(s: &Snapshot, fresh_only: bool, now: u64) -> Option<UsageSnapshot> {
    if s.validate().is_err()
        || s.provider != Provider::Antigravity
        || s.source != "statusline"
        || s.account_id != "active"
    {
        return None;
    }
    let state = s.effective_status(now);
    if fresh_only && state != DataStatus::Live {
        return None;
    }
    let windows = s
        .buckets
        .iter()
        .filter_map(|b| {
            b.remaining_fraction.map(|remaining| LimitWindow {
                id: b.id.clone(),
                label: b.label.clone(),
                used: 1.0 - remaining,
                resets_at: b.resets_at_ms,
                count: None,
                derived: b.estimated,
            })
        })
        .collect::<Vec<_>>();
    if windows.is_empty() {
        return None;
    }
    Some(UsageSnapshot {
        status: if state == DataStatus::Live {
            "ok"
        } else {
            "stale"
        }
        .into(),
        windows,
        fetched_at: s.observed_at_ms,
        note: format!("Antigravity CLI [active] · {state:?}"),
        backoff_until: 0,
    })
}

/// Persist the already-collected legacy cache, and wake Antigravity on new CLI input.
/// No additional remote provider polling is introduced by this worker.
pub fn start() {
    std::thread::spawn(|| {
        let root = match data_root() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Nyrva telemetry: {e}");
                return;
            }
        };
        let db = match History::open(&cli::database_path(&root)) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("Nyrva telemetry: {e}");
                return;
            }
        };
        let mut last_cli = None;
        let mut last_import = Instant::now() - Duration::from_secs(30);
        let mut last_prune = Instant::now() - Duration::from_secs(3600);
        let mut last_error = None;
        loop {
            let now = cli::now_ms();
            if let Ok(latest) = db.latest(now) {
                if let Some(s) = latest.iter().find(|s| {
                    s.provider == Provider::Antigravity
                        && s.source == "statusline"
                        && s.account_id == "active"
                }) {
                    let signature = (s.observed_at_ms, s.status);
                    if last_cli != Some(signature) {
                        last_cli = Some(signature);
                        crate::antigravity::request_refresh();
                    }
                }
            }
            if last_import.elapsed() >= Duration::from_secs(30) {
                let result = cli::read_current(&root, now).and_then(|items| {
                    for mut s in items
                        .into_iter()
                        .filter(|s| s.source == "legacy_adapter" && s.observed_at_ms > 0)
                    {
                        // The same validated ingestion boundary as statusline helpers.
                        config::record(&root, &mut s)?;
                    }
                    Ok(())
                });
                match result {
                    Err(e) => {
                        if last_error.as_ref() != Some(&e) {
                            eprintln!("Nyrva telemetry: {e}");
                        }
                        last_error = Some(e);
                    }
                    Ok(()) => last_error = None,
                }
                last_import = Instant::now();
            }
            if last_prune.elapsed() >= Duration::from_secs(3600) {
                let result = config::load(&root).and_then(|settings| {
                    db.prune(now.saturating_sub(u64::from(settings.retention_days) * 86_400_000))
                });
                if let Err(e) = result {
                    eprintln!("Nyrva telemetry: {e}");
                }
                last_prune = Instant::now();
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notch_projection_preserves_zero_reset_and_observation_time() {
        let s = nyrva_core::parse_statusline(
            Provider::Antigravity,
            "active",
            r#"{"quota":{"weekly":{"remaining_fraction":0,"reset_in_seconds":3600}}}"#,
            1000,
        )
        .unwrap();
        let projected = from_snapshot(&s, true, 1000).unwrap();
        assert_eq!(projected.windows[0].used, 1.0);
        assert_eq!(projected.windows[0].resets_at, Some(3_601_000));
        assert_eq!(projected.fetched_at, 1000);
        assert!(from_snapshot(&s, true, 700_000).is_none());
        assert_eq!(from_snapshot(&s, false, 700_000).unwrap().status, "stale");
    }
    #[test]
    fn notch_never_switches_accounts_implicitly() {
        let s = nyrva_core::parse_statusline(
            Provider::Antigravity,
            "work",
            r#"{"quota":{"weekly":{"remaining_fraction":0.9}}}"#,
            1000,
        )
        .unwrap();
        assert!(from_snapshot(&s, true, 1000).is_none());
    }
}
