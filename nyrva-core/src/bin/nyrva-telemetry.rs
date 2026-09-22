//! Optional standalone telemetry executable; the desktop uses the same command module.
use std::{io, path::PathBuf, process::ExitCode};
fn root() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("NYRVA_DATA_DIR").filter(|s| !s.is_empty()) { return Ok(PathBuf::from(path)); }
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    base.map(|p| p.join("nyrva")).ok_or_else(|| "cannot determine Nyrva data directory; set NYRVA_DATA_DIR".into())
}
fn main() -> ExitCode {
    let result = root().and_then(|root| nyrva_core::cli::run(&std::env::args().skip(1).collect::<Vec<_>>(), &root, &mut io::stdin().lock(), &mut io::stdout().lock()));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("Nyrva telemetry: {e}"); ExitCode::FAILURE }
    }
}
