# Everywhere implementation plan

> Execute inline with superpowers:executing-plans. This extends the approved three-macrostep plan; it does not authorize a main merge, release tag or publication.

**Goal:** Finish the local terminal/integration/API surfaces and fail-closed update/migration controls, then prove the combined application on Windows and Linux X11.
**Architecture:** Reuse the sanitized `nyrva-core` views; add focused terminal, integration, API, private-file and maintenance modules. No provider request is made by any of these commands.
**Stack:** Rust 2021, existing ring/serde/rusqlite/url/semver dependency versions; OS file locks and private permissions.
**Spec:** `docs/superpowers/specs/2026-09-22-nyrva-1.0-design.md`.

## Global constraints and review focus

Windows + Linux X11; preserve notch/Experience, licenses, provider credentials and unrelated configuration. Missing data never becomes quota. API is optional, loopback-only, authenticated, bounded and read-only. Imports, integration edits and migration writes require explicit confirmation. No auto-installation, signing-key generation, tag, version-1.0 bump or publication.

Review hostile shell paths, duplicate JSON/HTTP fields, session lifetime/permissions, concurrent integration changes, unsupported schemas, reset/source mixing and signed-artifact tampering. Test these against real files, sockets and subprocesses rather than mocked providers.

## Tasks

1. **Terminal and entrypoints** — `terminal.rs`, `cli.rs`, standalone/desktop entrypoints. Test `top --once --json`, redirected plain output, invalid interval, and global `--data-dir` before adding handlers. Interactive top uses only cached views; `q`/Ctrl-C exit cleanly. Reuse status/resets/sessions; do not duplicate ingestion parsing.
2. **Integration manager** — `integrations.rs`, `private_fs.rs`. Test plan/no mutation; explicit apply/replace; idempotent install; changed-statusline refusal; unrelated field preservation; hostile paths, JSON and symlinks; actual generated command execution. Receipts contain only the owned previous field, are private/nonportable, and are written before configuration replacement for crash recovery. Never execute an old command.
3. **Authenticated local API and event stream** — `local_api.rs`. Test unauthorized/origin/Host/method/duplicate headers, bounded requests and clients, two concurrent server instances, token rotation and cleanup, SSE and readonly persistence. OS-random tokens live in private session files; use the same core queries as terminal/desktop.
4. **Maintenance** — `maintenance.rs`. Test missing trust fails closed, wrong signature/artifact/origin/target/version is refused, and genuine Ed25519 signatures pass. Verification never installs or executes artifacts. Configure trust only with explicit apply. Test transactional v1/v2 migration and lossless rollback, refusing collisions/future schemas.
5. **Distribution and documentation** — include standalone telemetry executable in platform bundles, extend black-box smoke for both binaries and integrations, publish a practical command/security guide. Preserve release guards and version identity.
6. **Acceptance and review** — run unfiltered workspace suites, presentation/browser tests, native Windows/X11 acceptance, package builds and isolated installed/extracted-package smoke. Review the full change; record real results and any physical-device limitations. No mocks counted as native evidence.

For every task: add behavioral regression, run and observe RED, implement, rerun the focused test and full relevant suite, record GREEN and commit. Final acceptance must report all failures, not just new tests. Test sources are the executable detailed contracts for each input class above.
