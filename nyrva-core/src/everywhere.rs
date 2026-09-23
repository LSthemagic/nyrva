//! Small shared dispatch and strict option parser for local Everywhere commands.
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
};
#[derive(Default)]
pub(crate) struct Flags(BTreeMap<String, String>);
impl Flags {
    pub(crate) fn parse(
        args: &[String],
        values: &[&str],
        switches: &[&str],
    ) -> Result<Self, String> {
        let mut result = Self::default();
        let mut i = 0;
        while i < args.len() {
            let name = &args[i];
            if result.0.contains_key(name) {
                return Err("duplicate command option".into());
            }
            let value = if values.contains(&name.as_str()) {
                i += 1;
                args.get(i)
                    .filter(|s| !s.starts_with("--"))
                    .ok_or("missing option value")?
                    .clone()
            } else if switches.contains(&name.as_str()) {
                String::new()
            } else {
                return Err("unknown or misplaced command option".into());
            };
            result.0.insert(name.clone(), value);
            i += 1;
        }
        Ok(result)
    }
    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }
    pub(crate) fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
    pub(crate) fn number(
        &self,
        key: &str,
        default: u64,
        min: u64,
        max: u64,
    ) -> Result<u64, String> {
        let n = self
            .get(key)
            .map(str::parse::<u64>)
            .transpose()
            .map_err(|_| "invalid numeric option")?
            .unwrap_or(default);
        if !(min..=max).contains(&n) {
            return Err("numeric option outside its supported range".into());
        }
        Ok(n)
    }
}
pub(crate) fn run(
    args: &[String],
    root: &Path,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> Option<Result<(), String>> {
    Some(match args.first()?.as_str() {
        "top" => crate::terminal::run(&args[1..], root, out),
        "integrations" => crate::integrations::run(&args[1..], root, out),
        "serve" => crate::local_api::run(&args[1..], root, out),
        "update" => crate::maintenance::update(&args[1..], root, input, out),
        "migrate" => crate::maintenance::migrate(&args[1..], root, out),
        _ => return None,
    })
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(crate) fn unhex(text: &str, length: usize) -> Result<Vec<u8>, String> {
    if text.len() != length * 2 || !text.is_ascii() {
        return Err("invalid hexadecimal field length".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| "invalid hexadecimal field".into())
        })
        .collect()
}
