# Macrostep 2 — Experience completed

## Current checkpoint

**Experience implementation and its scoped automated acceptance are complete.** Both outstanding Windows findings are closed with native execution evidence. This is not completion of Everywhere, the entire workspace, or release 1.0 acceptance.

- Repository: `LSthemagic/nyrva`.
- Base: PR #18 merged to main at `fe1e88f2517d856dee899390e51709226a6ec92d`.
- Implementation branch: `fix/experience-completion`.
- Verified code/test commit: `fbe86ec61e87cdb1074a2837492e3f98153344cc`.
- Verified code tree: `e413f93f0df40e3baf6b3cca2f0490d5ac32d4de`.
- This checkpoint is a documentation-only successor to the verified code. Main, release versions, tags and publication were not modified in this execution.
- Scope: Macrostep 2 only. Stop here; do not begin Macrostep 3 without a separate instruction.

Plan: `docs/superpowers/plans/2026-09-22-nyrva-1.0.md`.
Spec: `docs/superpowers/specs/2026-09-22-nyrva-1.0-design.md`.

## Completed tasks

- [x] Diagnose and fix Windows native smoke startup before claiming graphical acceptance.
- [x] Test and correct the production window-opening boundary, including event-loop deferral, hide/reopen and repeated requests.
- [x] Re-run presentation, browser, shared facade and native acceptance on Windows and Linux X11, then review the actual diff.
- [x] Preserve and report the four existing Everywhere failures rather than suppressing them.

## Finding 1: Windows native smoke startup — closed

The old example exited with `-1073741511` before writing its report. Baseline run `35858633715`, Windows job `107173097980`, reproduced that failure after successful compilation and desktop tests.

The first diagnostic inspected DLLs inside Python, which has its own activation context; that result could not identify the standalone example's selected Common Controls version. The corrected, read-only PE diagnostic inspected the example's embedded resources and imports directly.

RED run `35859515310`, Windows job `107175988186`, reported `embedded_manifests: []`, `common_controls_v6: false`, and an import of `TaskDialogIndirect`, which System32 comctl32.dll did not export. The explicit embedded-manifest assertion failed.

Fix commit `d36ca667af6d4e1766331da3dbfc4739a65c3ef5` adds a Common Controls v6 manifest to the Cargo example using Windows MSVC example linker arguments in `nyrva/build.rs`. It requests `asInvoker`, not administrator privileges. The application already receives its own Tauri resources; the missing resource was in the test example. No DLL substitution, PATH manipulation, provider configuration change or runtime workaround was introduced.

The corrected example started and reached real WebView2 assertions in run `35860022749`. The final run below passes both the manifest assertion and all native smoke checks.

Primary references: [Microsoft TaskDialogIndirect requirements](https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-taskdialogindirect), [Cargo example linker arguments](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-link-arg-examples).

## Finding 2: synchronous event-loop window creation — closed

The production tray still calls the same public `observatory::open` entrypoint, but that function now accepts/coalesces requests and performs creation/restoration on a named worker thread. An atomic flag permits only one in-flight opening worker; a drop guard releases the flag on completion, construction failure or spawn failure. Existing-window reuse and close-to-hide behavior are preserved. Worker failures emit a static notice without source-provided metadata.

Test-first evidence: commit `375a7e1f3e5dd18b821384508f9505590f569fb9`, run `35860487052`, Linux job `107179209191`, failed the deterministic requirement with `window creation ran inline on the event-loop callback`. This proves the old synchronous behavior, not an observed deadlock. The [Tauri API warning](https://docs.rs/tauri/latest/x86_64-pc-windows-msvc/tauri/webview/struct.WebviewWindowBuilder.html#method.new) identifies the Windows deadlock hazard.

Fix commit `fbe86ec61e87cdb1074a2837492e3f98153344cc` moves creation out of that callback. The unchanged deferral assertion and lifecycle checks pass in the final native run on both platforms.

During test development, an assertion incorrectly expected the application to contain only one window. The standard Tauri configuration also creates the hidden notch. The corrected assertion compares the complete before/after window-label sets and requires exactly one dashboard plus preservation of the notch; it does not hide a product duplication defect.

## Exact-code verification

[Experience completion run 35861118995](https://github.com/LSthemagic/nyrva/actions/runs/35861118995), code commit `fbe86ec61e87cdb1074a2837492e3f98153344cc`:

| Check | Job | Actual result |
| --- | --- | --- |
| Presentation contracts and browser E2E | `107181303805` | PASS: 10 format contracts and 10 browser scenarios |
| Linux X11 Experience acceptance | `107181303895` | PASS: compile, 45 desktop tests, 6 facade tests, native example build and 8 native checks |
| Windows Experience acceptance | `107181303922` | PASS: compile, 27 desktop tests, 6 facade tests, native example build, embedded manifest and 8 native checks |
| Full core regression, Linux | `107181303477` | FAIL: exactly the four existing Everywhere contracts |
| Full core regression, Windows | `107181303905` | FAIL: exactly the same four existing Everywhere contracts |

Both native jobs execute `cargo check -p nyrva --locked`, `cargo test -p nyrva --locked`, `cargo test -p nyrva-core --locked --test experience_bridge`, `cargo build -p nyrva --example experience-smoke --locked`, and the unchanged-lockfile check. Linux's 45 desktop tests comprise 41 unit tests and four integration tests; Windows's 27 comprise 23 and four. No ignored or filtered tests were reported in those suites.

The native harness imports the actual production opener and plugin, uses the packaged dashboard HTML/modules and real SQLite, and refuses an already-existing test data directory. It invokes the opener from a synchronous event-loop callback after setup. Its eight checks cover safe source-text/zero rendering, scoped native reads/revisions, persisted preferences/live rendering, close/hide/reopen and 32 rapid opening requests preserving notch/layout, invalid-import rejection, account/source-scoped history, rejected/cancelled clearing, and confirmed UI clearing with preferences preserved. Both reports contain `ok: true`, `creation_deferred: true`, `opening_callback_returned: true` and `provider_file_preserved: true`.

Browser E2E separately uses injected IPC fixtures; it is not counted as native acceptance. It checks expired-reset/zero rendering, hostile metadata, reported sessions/context/projects/agents, history tables, layouts, dirty-form preservation, silence, failures retaining the last read, keyboard tabs/focus, privacy confirmation/cancellation, empty state and a narrow viewport.

Artifacts in that run, with seven-day retention:

- `experience-completion-browser`, ID `10750236319`: screenshots, trace and result JSON.
- `experience-completion-Linux`, ID `10749803883`: native report, synthetic evidence and build/test logs.
- `experience-completion-Windows`, ID `10750286668`: native report, PE diagnostics and build/test logs.
- `experience-full-core-Linux`, ID `10750291295`, and `experience-full-core-Windows`, ID `10749748774`: unfiltered full-core regression logs.

The workflow and test source remain reproducible after artifact expiry. CI is still globally red because the full-core jobs retain their real exit codes; do not describe the whole workflow or workspace as green.

## Product and privacy review

The previously merged dashboard, three layouts, independent quota/reset/source views, reported sessions/context/projects/agents, conservative history, deduplicated alerts, settings and Privacy Center are retained. No feature was replaced with mocked product data. This follow-up changes the production window entrypoint and the example's build resources, plus tests/CI/documentation; provider adapters, authentication files, schema, Cargo.lock and release machinery are unchanged.

Review checked request coalescing, error recovery, main-thread ownership, reuse of the same entrypoint in production/tests, test-example-only reporting commands, source-text rendering and preservation of provider data. Review was performed inline; no independent reviewer subagent was available.

Privacy settings retain their documented meaning: inclusion controls observatory views/new history, not legacy notch polling; disabling metadata does not erase old rows; clearing Nyrva history preserves preferences/provider files and new collection can repopulate it.

## Release acceptance and Everywhere remain outside this closure

No physical mouse click on the OS tray, physical multi-monitor/DPI matrix, comprehensive screen-reader audit, real-provider authentication, installed package or updater/signature acceptance was performed here. The native smoke tests actual webview/IPC/SQLite/window behavior on CI Windows and X11, not the complete release installation experience. Preserve those distinctions during Macrostep 3 and final release validation.

Full-core regression still fails only these existing tests in `everywhere_contracts`:

- `doctor_and_redirected_top_are_readonly_and_versioned`
- `integration_is_opt_in_and_restores_only_its_owned_field`
- `integration_refuses_to_overwrite_a_statusline_changed_after_install`
- `local_api_requires_session_auth_has_no_cors_and_rejects_writes`

No handlers or tests for Everywhere were implemented, removed, ignored or weakened in this task. All other full-core targets completed successfully; the failed target reports 6 passed/4 failed on Linux and 5 passed/4 failed on Windows because of existing platform-specific coverage.

## Resume instructions

Experience is closed at this checkpoint. Preserve the branch and consume the follow-up changes before starting the next macrostep. Do not reset to the old feature branch or repeat the completed UI/Windows work. Read the current remote main/PR state before selecting a base. Start Everywhere only when separately requested, then complete its unimplemented handlers and full release acceptance before proposing 1.0 publication.

Execution used actual GitHub Actions because local container/Python calls returned ClientError. No local test execution, independent review, main merge or release publication is claimed.
