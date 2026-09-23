//! Opt-in ownership of one statusLine field. Receipts are private and never exported.
use crate::{
    cli,
    everywhere::{hex, Flags},
    private_fs, storage, validate_account,
};
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_json::{json, Value};
use std::{
    fmt, fs,
    io::Write,
    path::{Path, PathBuf},
};
const LIMIT: usize = 256 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    provider: String,
    target: PathBuf,
    previous_present: bool,
    previous: Value,
    installed: Value,
}
// Reject duplicate keys recursively. Rewriting ambiguous JSON could destroy unrelated settings.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("unambiguous JSON")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut m: M) -> Result<Unique, M::Error> {
                let mut out = serde_json::Map::new();
                while let Some((k, v)) = m.next_entry::<String, Unique>()? {
                    if out.insert(k, v.0).is_some() {
                        return Err(de::Error::custom("duplicate key"));
                    }
                }
                Ok(Unique(Value::Object(out)))
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut s: S) -> Result<Unique, S::Error> {
                let mut out = Vec::new();
                while let Some(v) = s.next_element::<Unique>()? {
                    out.push(v.0);
                }
                Ok(Unique(Value::Array(out)))
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Unique, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
        }
        deserializer.deserialize_any(V)
    }
}
fn settings(bytes: Option<&[u8]>) -> Result<Value, String> {
    let v = match bytes {
        Some(b) => {
            serde_json::from_slice::<Unique>(b)
                .map_err(|_| "provider settings are invalid or contain duplicate keys")?
                .0
        }
        None => json!({}),
    };
    if !v.is_object() {
        return Err("provider settings must be a JSON object".into());
    }
    Ok(v)
}
fn path_text(path: &Path) -> Result<String, String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("integration paths must be absolute without parent traversal".into());
    }
    let text = path.to_str().ok_or("integration paths must be Unicode")?;
    if text.chars().any(|c| c.is_control()) {
        return Err("integration paths cannot contain control characters".into());
    }
    #[cfg(windows)]
    {
        if text
            .chars()
            .any(|c| matches!(c, '"' | '%' | '!' | '$' | '`' | '&' | '|' | '<' | '>' | '^'))
            || text.starts_with("\\\\")
        {
            return Err("integration path is unsafe for the Windows command shell".into());
        }
        Ok(format!("'{}'", text.replace('\\', "/").replace('\'', "''")))
    }
    #[cfg(not(windows))]
    {
        Ok(format!("'{}'", text.replace('\'', "'\"'\"'")))
    }
}
fn home() -> Result<PathBuf, String> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| "cannot resolve home; pass --home".into())
}
pub(crate) fn run(args: &[String], root: &Path, out: &mut dyn Write) -> Result<(), String> {
    let action = args
        .first()
        .map(String::as_str)
        .ok_or("integrations requires plan, install or remove")?;
    if !["plan", "install", "remove"].contains(&action) {
        return Err("unknown integration action".into());
    }
    let provider = args
        .get(1)
        .map(String::as_str)
        .ok_or("integration requires a provider")?;
    let relative = match provider { "claude" => ".claude/settings.json", "antigravity" => ".gemini/antigravity-cli/settings.json", _ => return Err("custom statusline integration is supported only for Claude and Antigravity; no slot is invented for other providers".into()) };
    let flags = Flags::parse(
        &args[2..],
        &["--home", "--executable", "--account"],
        &["--apply", "--replace", "--json"],
    )?;
    if action != "install" && flags.has("--replace") {
        return Err("--replace is only valid with install".into());
    }
    if action == "plan" && flags.has("--apply") {
        return Err("plan is read-only".into());
    }
    if action != "plan" && !flags.has("--apply") {
        return Err("integration mutation requires --apply".into());
    }
    let home = flags
        .get("--home")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(home)?;
    path_text(&home)?;
    let target = home.join(relative);
    let original = private_fs::read(&target, LIMIT)?;
    let mut current = settings(original.as_deref())?;
    let account = flags.get("--account").unwrap_or("active");
    validate_account(account)?;
    let installed = if action != "remove" {
        let executable = flags
            .get("--executable")
            .map(PathBuf::from)
            .map(Ok)
            .unwrap_or_else(|| {
                std::env::current_exe().map_err(|_| "cannot locate telemetry executable")
            })?;
        if !executable.is_file() {
            return Err("integration executable does not exist".into());
        }
        let native = format!(
            "{} --data-dir {} statusline {provider} --account {account}",
            path_text(&executable)?,
            path_text(root)?
        );
        // Claude uses Git Bash or PowerShell on Windows. A quoted executable alone
        // is an expression in PowerShell; a fixed PowerShell launcher works in both.
        // Paths reject outer-shell expansion characters and use PS literal quoting.
        #[cfg(windows)]
        let command = format!("powershell.exe -NoProfile -NonInteractive -Command \"& {native}\"");
        #[cfg(not(windows))]
        let command = native;
        json!({"type":"command","command":command})
    } else {
        Value::Null
    };
    if action == "plan" {
        return cli::write_json(
            out,
            &json!({"schema_version":1,"provider":provider,"settings_file":relative,"owned_field":"statusLine","requires_confirmation":true,"replacement_required":current.get("statusLine").is_some(),"authentication_used":false,"executes_existing_command":false}),
        );
    }
    let id = hex(
        ring::digest::digest(&ring::digest::SHA256, target.to_string_lossy().as_bytes()).as_ref(),
    );
    let dir = storage::state_dir(root).join("integrations");
    let _lock = private_fs::lock(&dir.join(format!("{id}.lock")))?;
    // Recheck after acquiring the operation lock, before any replacement.
    if private_fs::read(&target, LIMIT)? != original {
        return Err(
            "provider settings changed during planning; retry without overwriting them".into(),
        );
    }
    let receipt_path = dir.join(format!("{id}.json"));
    let receipt = private_fs::read(&receipt_path, LIMIT * 2)?
        .map(|b| serde_json::from_slice::<Receipt>(&b).map_err(|_| "invalid integration receipt"))
        .transpose()?;
    if let Some(r) = &receipt {
        if r.schema_version != 1 || r.target != target || r.provider != provider {
            return Err("integration receipt does not match this target".into());
        }
    }
    if action == "install" {
        let receipt = if let Some(r) = receipt {
            if current.get("statusLine") == Some(&r.installed) {
                if r.installed != installed {
                    return Err("integration already installed with different options; remove it explicitly first".into());
                }
                return cli::write_json(
                    out,
                    &json!({"schema_version":1,"installed":true,"changed":false,"provider":provider}),
                );
            }
            let previous_matches = if r.previous_present {
                current.get("statusLine") == Some(&r.previous)
            } else {
                current.get("statusLine").is_none()
            };
            if !previous_matches || r.installed != installed {
                return Err(
                    "statusline changed after installation; refusing to overwrite it".into(),
                );
            }
            r
        } else {
            if current.get("statusLine").is_some() && !flags.has("--replace") {
                return Err(
                    "existing statusline preserved; replacement requires --apply --replace".into(),
                );
            }
            Receipt {
                schema_version: 1,
                provider: provider.into(),
                target: target.clone(),
                previous_present: current.get("statusLine").is_some(),
                previous: current.get("statusLine").cloned().unwrap_or(Value::Null),
                installed,
            }
        };
        private_fs::write(
            &receipt_path,
            &serde_json::to_vec(&receipt).map_err(|_| "cannot encode integration receipt")?,
        )?;
        current["statusLine"] = receipt.installed;
    } else {
        let r = receipt.ok_or("no owned integration exists for this target")?;
        if current.get("statusLine") != Some(&r.installed) {
            let restored = if r.previous_present {
                current.get("statusLine") == Some(&r.previous)
            } else {
                current.get("statusLine").is_none()
            };
            if !restored {
                return Err(
                    "statusline changed after installation; refusing to overwrite it".into(),
                );
            }
            // An interruption after restoration leaves only our private receipt to remove.
            // Do not rewrite the provider file, even to normalize its formatting.
            fs::remove_file(&receipt_path)
                .map_err(|_| "cannot finish integration receipt cleanup")?;
            return cli::write_json(
                out,
                &json!({"schema_version":1,"installed":false,"changed":false,"recovered":true,"provider":provider}),
            );
        }
        if r.previous_present {
            current["statusLine"] = r.previous;
        } else {
            current
                .as_object_mut()
                .ok_or("invalid settings")?
                .remove("statusLine");
        }
    }
    if private_fs::read(&target, LIMIT)? != original {
        return Err("provider settings changed concurrently; receipt retained for recovery".into());
    }
    storage::atomic_write(
        &target,
        &serde_json::to_vec_pretty(&current).map_err(|_| "cannot encode provider settings")?,
    )?;
    if action == "remove" {
        fs::remove_file(receipt_path)
            .map_err(|_| "integration restored, but receipt cleanup failed")?;
    }
    cli::write_json(
        out,
        &json!({"schema_version":1,"provider":provider,"installed":action=="install","changed":true,"scope":"statusLine_only","provider_authentication_modified":false}),
    )
}
