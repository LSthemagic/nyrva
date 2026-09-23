# Nyrva

**AI coding usage, sessions and limits — visible where you work.**

Nyrva is a desktop and terminal companion for monitoring AI coding assistants without constantly switching apps. It brings provider usage, reset windows, sessions, projects and agent activity into a compact desktop experience, a terminal dashboard and a local read-only API.

Supports **Windows** and **Linux X11**, with **Claude Code, Codex, Cursor and Antigravity**.

> Nyrva is an independent fork of [Codenotch](https://github.com/vinzdg/codenotch). See [credits and provenance](CREDITS.md).

## Download

| Platform | Package |
|---|---|
| Windows x64 | `Nyrva_1.0.0_x64-setup.exe` |
| Debian / Ubuntu x86_64 | `Nyrva_1.0.0_amd64.deb` |
| Linux x86_64 portable | `Nyrva_1.0.0_amd64.AppImage` |

[**Download Nyrva v1.0.0**](https://github.com/LSthemagic/nyrva/releases/tag/v1.0.0)

The 1.0 candidate is initially kept as a draft while physical acceptance is completed. Packages are currently unsigned; use the included `SHA256SUMS` to verify integrity.

## What Nyrva 1.0 gives you

- **Desktop observatory** — quotas, reset windows, sessions, projects, agents and alerts while keeping the compact edge notch.
- **Terminal dashboard** — run `nyrva-telemetry top` without opening the desktop app.
- **Scriptable JSON** — query status, history, resets, sessions, projects, agents and diagnostics.
- **Statusline integrations** — reversible, opt-in integrations for supported Claude and Antigravity statuslines.
- **Local API + SSE** — explicit, authenticated, loopback-only and read-only access for your own local tools.
- **Privacy controls** — local storage boundaries, sanitized diagnostics, portable settings and Nyrva-only cleanup.
- **Safe maintenance** — transactional migrations, settings rollback and fail-closed artifact verification.
- **Cross-platform packages** — NSIS on Windows; Debian and AppImage on Linux X11.

Nyrva reads provider-owned local state and credentials as read-only inputs. It does not replace provider login or silently refresh credentials.

## Quick start

Install the package for your platform, sign in through the provider applications you already use, then launch Nyrva.

For the terminal:

```sh
nyrva-telemetry status --json
nyrva-telemetry top
nyrva-telemetry sessions --json
nyrva-telemetry projects --json
nyrva-telemetry doctor --json
```

On Windows, use `nyrva-telemetry.exe` in PowerShell/cmd when the installation directory is not already on `PATH`.

## Documentation

**Using Nyrva**

- 🇧🇷 [Guia completo em Português](docs/guide.pt-BR.md)
- 🇺🇸 [Complete English guide](docs/guide.en.md)
- [Everywhere CLI, integrations and local API reference](docs/everywhere.md)

**Project / maintainers**

- [Development](docs/DEVELOPMENT.md)
- [Manual release smoke tests](docs/SMOKE_TESTS.md)
- [Releasing](docs/RELEASING.md)
- [Credits](CREDITS.md)

## Scope

Nyrva 1.0 targets **Windows x64** and **Linux x86_64 on X11**. Wayland and macOS are not part of the 1.0 support target.

Provider availability depends on each provider's installed application, local state and account capabilities. Missing data is treated as unavailable — not as zero usage.

## Architecture

Nyrva is built with Rust and Tauri 2.

```text
Cargo.toml
├── nyrva/       # desktop app + shared command entrypoint
├── nyrva-core/  # local data, privacy, CLI and Everywhere surfaces
└── nyrva-hook/  # Claude Code hook launcher
```

## License & attribution

Nyrva is MIT licensed and derives from [Codenotch](https://github.com/vinzdg/codenotch) by **Vinz (@vinzdg) and contributors**, including the existing **Rust/Tauri Windows port credited to Im-Midi (NG) and contributors**.

See [CREDITS.md](CREDITS.md), [LICENSE](LICENSE) and the provider glyph notices shipped with the packages.
