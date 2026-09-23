# Nyrva

Nyrva is a lightweight desktop usage monitor for AI coding assistants. It pins provider usage and session activity to the edge of your screen so limits stay visible without interrupting your workflow.

An independent fork of [Codenotch](https://github.com/vinzdg/codenotch), built on its existing Windows port. See [credits and provenance](CREDITS.md).

## Nyrva 1.0 scope and release status

The implementation targets **Windows and Linux X11**, with **Claude Code, Codex, Cursor and Antigravity**. Windows packages use NSIS (`.exe`); Linux packages use `.deb` and AppImage. Each package includes the `nyrva-hook` helper and the independent `nyrva-telemetry` console executable.

**Implementation and automated packaging are not desktop acceptance.** The public 1.0 release remains blocked until the [manual smoke checklist](docs/SMOKE_TESTS.md) is executed and recorded on Windows and a real Linux X11 session. No manual pass is implied by this README or by a green CI run.

Ubuntu 22.04 is the Linux CI build baseline; Ubuntu 22.04 and Debian 12 are the intended Linux baselines. Record the exact environments actually tested rather than assuming all distributions work.

Wayland, macOS, new providers and automatic installation of application updates are outside this release. The existing notch remains available alongside the desktop observatory.

## Core, Experience and Everywhere

The three implementation macrosteps share a single local-data boundary:

- **Core:** normalized provider observations, independent quota windows, sessions, bounded history and redacted diagnostics.
- **Experience:** desktop cockpit, sessions, projects, agent views, reset visibility, alert policies and privacy controls, without replacing the notch.
- **Everywhere:** a headless console, cached terminal dashboard, reversible opt-in statuslines, authenticated read-only loopback API/SSE, portable preferences, lossless migration controls and signed local-artifact verification.

Start with the [Everywhere command and security guide](docs/everywhere.md). Use `nyrva-telemetry help` for console commands; on Windows prefer `nyrva-telemetry.exe` for terminal/scripts. The desktop executable delegates supported commands to the same core. Neither a statusline nor the local API performs provider authentication or remote collection on each request.

[Implementation acceptance](docs/everywhere-acceptance.md) records the tested commit, automated evidence and the physical-device checks still owned by the maintainer. It is separate from public release authorization.

## Installable builds

Successful CI runs upload:

- `nyrva-windows`: Windows NSIS installer.
- `nyrva-linux`: Debian package and AppImage.

For development testing, download the artifacts from the successful Actions run for the commit being tested. These are CI builds, not a declaration of a stable release.

Version tags matching `v*`, or a reviewed change to `.github/release-request.json` merged into `main`, run the [release workflow](.github/workflows/release.yml). It validates the requested version against Cargo and Tauri, reuses the same CI build/test/package gates, requires all three package formats and creates a **draft GitHub Release** with `SHA256SUMS`. For a main-branch request, it creates the matching tag at the exact built commit only after the checks pass. A maintainer publishes the draft only after manual acceptance; pushing a tag does not publish a stable release automatically.

See [release instructions](docs/RELEASING.md) for versioning, acceptance, publishing and failed-run recovery. Packages are unsigned; checksums verify integrity, not publisher identity.

## Provider requirements

Sign in using each provider's own application first. Nyrva is not an authentication manager and must not refresh or modify provider credentials. Provider-owned credentials/databases are read-only inputs; explicitly installing or removing a hook/statusline integration changes only its managed configuration. Statusline replacement requires explicit confirmation and preserves unrelated provider settings.

Codex uses `CODEX_HOME` or `~/.codex`; Cursor uses its local state database; Antigravity prefers the running local language-server bridge and has credential/transcript fallbacks. Provider presence, local state, service availability and account capabilities affect what can be displayed. Missing data must not be interpreted as zero usage.

See [development and provider notes](docs/DEVELOPMENT.md) for prerequisites and limitations.

## Architecture

Nyrva uses Rust and Tauri 2. The compact edge-notch UI is retained from the existing Windows port.

```text
Cargo.toml
├── nyrva/       # Tauri desktop application and shared-command entrypoint
├── nyrva-core/  # Shared local data, privacy, CLI and Everywhere surfaces
└── nyrva-hook/  # Claude Code hook launcher
```

OS-specific behavior is isolated behind the platform layer. Provider adapters live in the desktop crate.

## Development

Install the platform prerequisites in [DEVELOPMENT.md](docs/DEVELOPMENT.md). Prepare the sidecar **before** the first workspace check/test.

Windows, from the repository root:

```powershell
./scripts/prepare-windows-sidecar.ps1 debug
cargo check --workspace
cargo test --workspace
```

Linux, from the repository root:

```bash
bash scripts/prepare-linux-sidecar.sh debug
cargo check --workspace
cargo test --workspace
```

Release tooling tests use Python 3.11+ and only its standard library:

```bash
python -m unittest discover -s tests -p 'test_*.py' -v
```

Build/package commands, manual test records and publication steps are documented separately so passing a build cannot be mistaken for release acceptance. Documentation in `docs/DEVELOPMENT.md`, `docs/SMOKE_TESTS.md` and `docs/RELEASING.md` is in Portuguese.

## License and attribution

Nyrva is MIT licensed and derives from [Codenotch](https://github.com/vinzdg/codenotch) by **Vinz (@vinzdg) and contributors**, including the existing **Rust/Tauri Windows port credited to Im-Midi (NG) and contributors**. The original concept, prior code and derived artwork remain attributed to their authors; this fork does not claim they were created from scratch here.

Nyrva adds its workspace/branding, platform separation, Linux X11 adaptations and cross-platform delivery work. See [CREDITS.md](CREDITS.md) for provenance and [LICENSE](LICENSE) for the inherited copyright and permission notice. `LICENSE`, `CREDITS.md` and the provider glyph notice are included in the application packages and attached to the release.
