# Nyrva 1.0 User Guide

User guide for **Windows** and **Linux X11**. Build and release procedures live in the maintainer documentation.

## 1. What Nyrva does

Nyrva brings local AI coding assistant information into three surfaces:

1. **Desktop** — compact notch + observatory for quotas, resets, sessions, projects, agents and alerts.
2. **Terminal** — `nyrva-telemetry` for a dashboard and JSON queries.
3. **Local integrations** — opt-in statuslines and an authenticated, read-only local HTTP/SSE API.

Nyrva 1.0 supports **Claude Code, Codex, Cursor and Antigravity**. Nyrva does not sign in for you; authenticate normally in each provider's own app/CLI.

## 2. Install

Download from [Releases](https://github.com/LSthemagic/nyrva/releases/tag/v1.0.0).

### Windows

Download `Nyrva_1.0.2_x64-setup.exe` and run the installer. The package includes the `nyrva.exe` desktop app, `nyrva-telemetry.exe` CLI and `nyrva-hook.exe` helper.

1.0 packages are currently unsigned. Do not disable Windows protections; verify the source and compare the package against `SHA256SUMS`.

### Debian / Ubuntu on X11

```bash
sudo apt install ./Nyrva_1.0.2_amd64.deb
```

Run Nyrva as your normal user, not root.

### AppImage

```bash
chmod +x ./Nyrva_1.0.0_amd64.AppImage
./Nyrva_1.0.0_amd64.AppImage
```

Keep the AppImage at a stable path when using integrations that may relaunch it. Wayland is outside the 1.0 target.

## 3. First run

1. Open and authenticate the providers you use.
2. Start Nyrva.
3. Use the notch for essential at-a-glance state.
4. Open the observatory for quotas, resets, sessions, projects and agents.
5. Missing information should appear unavailable/stale — missing data does not mean a 0% quota.

Codex honors `CODEX_HOME` or `~/.codex`; Cursor uses its local database; Antigravity prefers its local bridge and has fallbacks; Claude can use Nyrva's integration/hook.

## 4. Terminal

The CLI works without opening the GUI:

```bash
nyrva-telemetry status --json
nyrva-telemetry resets --json
nyrva-telemetry sessions --json
nyrva-telemetry cockpit --json
nyrva-telemetry projects --json
nyrva-telemetry agents --json
nyrva-telemetry history antigravity --account active --json
nyrva-telemetry doctor --json
nyrva-telemetry export --json
```

Interactive dashboard:

```bash
nyrva-telemetry top
```

Controls: `s` + Enter for sessions, `p` + Enter for quotas, `q` + Enter or Ctrl-C to quit.

Non-interactive snapshot:

```bash
nyrva-telemetry top --once --json
```

Public JSON output uses `schema_version: 1`.

## 5. Data and isolated testing

Defaults: `%APPDATA%/nyrva` on Windows; `$XDG_CONFIG_HOME/nyrva` or `~/.config/nyrva` on Linux.

```bash
nyrva-telemetry --data-dir /absolute/test/path status --json
```

`NYRVA_DATA_DIR` is also supported; `--data-dir` takes precedence. Do not experiment with destructive maintenance commands against your everyday data directory.

## 6. Claude and Antigravity statuslines

Installation is **opt-in**. Start with `plan`, which does not write files.

Linux:

```bash
nyrva-telemetry integrations plan claude --executable /usr/bin/nyrva-telemetry --json
nyrva-telemetry integrations install claude --executable /usr/bin/nyrva-telemetry --apply
nyrva-telemetry integrations remove claude --apply
```

Windows PowerShell:

```powershell
$cli = (Resolve-Path '.\nyrva-telemetry.exe').Path
& $cli integrations plan claude --executable $cli --json
& $cli integrations install claude --executable $cli --apply
& $cli integrations remove claude --apply
```

Replace `claude` with `antigravity`. If a statusline already exists, Nyrva requires explicit `--replace`. Unrelated fields are preserved; if you manually edit the managed statusline later, Nyrva refuses to silently overwrite your change.

### Codex

Codex uses its own native status line. Nyrva adds the native `five-hour-limit` and `weekly-limit` indicators without replacing the user's other selected items:

```powershell
nyrva-telemetry integrations plan codex
nyrva-telemetry integrations install codex --apply
nyrva-telemetry integrations remove codex --apply
```

The change goes through the official `codex app-server` API with config version checks and effective-config verification. `remove` removes only the two indicators managed by Nyrva. Reopen Codex to see the updated line.

## 7. Settings, privacy and alerts

```bash
nyrva-telemetry settings export --json
nyrva-telemetry settings rollback --apply
nyrva-telemetry privacy status --json
nyrva-telemetry alerts list --json
nyrva-telemetry alerts refresh --json
nyrva-telemetry privacy clear --confirm
```

`privacy clear` removes Nyrva-owned data, not provider credentials/files. Settings import accepts the exported `settings` object and a previous settings version is available for rollback.

## 8. Migrations

```bash
nyrva-telemetry migrate status --json
nyrva-telemetry migrate --apply
nyrva-telemetry migrate rollback --apply
```

Migrations are transactional. A rollback that would lose or collide data is rejected.

## 9. Local API and SSE

```bash
nyrva-telemetry serve --port 0 --duration 3600
```

The API listens only on `127.0.0.1`, generates a token per session, requires `Authorization: Bearer <token>`, is read-only and does not enable CORS. The temporary address/token live in `telemetry/api-session.json`. **Do not share this file.**

Routes:

```text
/v1/status
/v1/cockpit
/v1/resets
/v1/sessions
/v1/projects
/v1/agents
/v1/doctor
/v1/history?provider=claude&account=active&source=statusline&limit=100
/v1/events
```

`/v1/events` provides sanitized SSE. The API is for local automation, not network exposure.

## 10. Diagnostics

```bash
nyrva-telemetry doctor --json
nyrva-telemetry status --json
nyrva-telemetry privacy status --json
```

If a provider is missing, verify it is installed/authenticated and has local state available. Never post `auth.json`, provider databases, tokens, private prompts or `api-session.json` in issues.

## 11. Updates and limitations

`nyrva-telemetry update status --json` reports trust/verification state. In 1.0, `update` verifies artifacts when explicit Ed25519 publisher trust is configured; it **does not automatically install updates**.

Current limitations: Windows x64 and Linux x86_64/X11; no official Wayland/macOS support; unsigned packages; provider-local formats can change.

For the full technical CLI/API reference, see [Everywhere](everywhere.md).
