#![cfg(windows)]
//! The provider passes the generated string to a shell, not the C argv parser.
use nyrva_core::cli;
use serde_json::{json, Value};
use std::{fs, io::Write, path::PathBuf, process::{Command, Stdio}, time::{Duration, Instant}};

struct Temp(PathBuf);
impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn execute(shell: &str) {
    let t = Temp(std::env::temp_dir().join(format!("nyrva-shell-{}-{}-{}", std::process::id(), cli::now_ms(), shell)));
    let root = t.0.join("dados com espaços");
    let home = t.0.join("test home");
    let bin_dir = t.0.join("app's directory");
    fs::create_dir_all(&bin_dir).unwrap();
    let exe = bin_dir.join("nyrva-telemetry.exe");
    fs::copy(env!("CARGO_BIN_EXE_nyrva-telemetry"), &exe).unwrap();
    let args = ["integrations", "install", "claude", "--home", home.to_str().unwrap(), "--executable", exe.to_str().unwrap(), "--apply"].map(str::to_owned);
    cli::run(&args, &root, &mut std::io::empty(), &mut Vec::new()).unwrap();
    let settings: Value = serde_json::from_slice(&fs::read(home.join(".claude/settings.json")).unwrap()).unwrap();
    let line = settings["statusLine"]["command"].as_str().unwrap();
    let mut c = match shell {
        "powershell" => { let mut c = Command::new("powershell.exe"); c.args(["-NoProfile", "-NonInteractive", "-Command", line]); c },
        "bash" => { let path = PathBuf::from(std::env::var_os("ProgramFiles").unwrap()).join("Git/bin/bash.exe"); assert!(path.exists(), "Git Bash fixture is required on this runner"); let mut c=Command::new(path); c.args(["--noprofile", "--norc", "-c", line]); c },
        "cmd" => { use std::os::windows::process::CommandExt; let mut c=Command::new("cmd.exe"); c.args(["/d", "/s", "/c"]).raw_arg(format!("\"{line}\"")); c },
        _ => unreachable!(),
    };
    let mut child=c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let input=json!({"rate_limits":{"five_hour":{"used_percentage":100}},"model":{"display_name":"Modelo Ação"}}).to_string();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let deadline=Instant::now()+Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now()>deadline { let _=child.kill(); let _=child.wait(); panic!("{shell} generated statusline timed out"); }
        std::thread::sleep(Duration::from_millis(30));
    }
    let output=child.wait_with_output().unwrap();
    assert!(output.status.success(), "{shell}: {}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("0%"), "{shell}: {}", String::from_utf8_lossy(&output.stdout));
    let mut out=Vec::new();
    cli::run(&["status".into(), "--json".into()], &root, &mut std::io::empty(), &mut out).unwrap();
    let state: Value=serde_json::from_slice(&out).unwrap();
    assert_eq!(state["providers"][0]["buckets"][0]["remaining_fraction"], 0.0);
    assert_eq!(state["providers"][0]["model"], "Modelo Ação");
}
#[test] fn powershell_executes_the_installed_command_with_utf8_and_spaces() { execute("powershell"); }
#[test] fn git_bash_executes_the_same_installed_command() { execute("bash"); }
#[test] fn cmd_executes_the_same_installed_command_without_crt_escaping() { execute("cmd"); }
