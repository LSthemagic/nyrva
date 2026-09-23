//! Real desktop entrypoints must dispatch locally before constructing Tauri.
use std::{
    fs,
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
#[test]
fn everywhere_read_commands_never_start_the_desktop_or_create_history() {
    let root = std::env::temp_dir().join(format!("nyrva-entrypoints-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let data = root.join("data");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    for args in [
        vec!["doctor", "--json"],
        vec!["top", "--once", "--json"],
        vec!["settings", "export", "--json"],
        vec!["migrate", "status", "--json"],
        vec!["update", "status", "--json"],
        vec![
            "integrations",
            "plan",
            "antigravity",
            "--home",
            home.to_str().unwrap(),
            "--json",
        ],
        vec!["--data-dir", data.to_str().unwrap(), "status", "--json"],
    ] {
        let mut p = Command::new(env!("CARGO_BIN_EXE_nyrva"))
            .args(&args)
            .env("NYRVA_DATA_DIR", &data)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("APPDATA", root.join("config"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(12);
        let status = loop {
            if let Some(s) = p.try_wait().unwrap() {
                break s;
            }
            if Instant::now() > deadline {
                let _ = p.kill();
                let _ = p.wait();
                panic!("{} started a GUI or hung", args[0]);
            }
            thread::sleep(Duration::from_millis(20));
        };
        let mut output = String::new();
        p.stdout
            .take()
            .unwrap()
            .read_to_string(&mut output)
            .unwrap();
        assert!(status.success(), "{} failed", args[0]);
        let result: serde_json::Value = serde_json::from_str(&output)
            .unwrap_or_else(|_| panic!("{} did not return canonical JSON", args[0]));
        assert_eq!(result["schema_version"], 1);
        assert!(!data.exists(), "read command {} mutated state", args[0]);
    }
    let _ = fs::remove_dir_all(root);
}
