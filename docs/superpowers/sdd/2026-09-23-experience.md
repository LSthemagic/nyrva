# Experience checkpoint — 2026-09-23

## Current execution boundary

User instruction: continue from the interruption, **one macrostep at a time**. This supersedes the earlier instruction to continue across all three macrosteps without pausing.

Scope of this execution: **Macrostep 2 — Experience only**. Status: **implementation committed; acceptance NOT closed**. Two Windows blockers remain below. Do not start Everywhere, merge to main, tag, or publish a release from this checkpoint.

- Repository: `LSthemagic/nyrva`.
- Branch: `feat/nyrva-1.0`.
- Resume parent: `e7d4ea6ea72dc6c9f621481098d4ee3bce6a9e28`.
- Latest code and test commit: `2ceed852cdf78929c8a623f3eb186b8d68e066f1`.
- Code tree: `c1a22268a8f9dccf197dd8befaa869bb3d67c038`.
- This checkpoint is a documentation-only successor; the CI evidence below refers to the exact code commit above.
- Main was not modified by this execution. All eight code/test commits are fast-forward descendants of the resume parent.

Plan: `docs/superpowers/plans/2026-09-22-nyrva-1.0.md`.
Spec: `docs/superpowers/specs/2026-09-22-nyrva-1.0-design.md`.

## Implemented and committed

1. A separate, focusable Experience dashboard, leaving the notch window policy unchanged. Six sections: overview, sessions, projects/agents, history, source health, and privacy/settings. Compact, bars, and reset-first layouts are persisted through the shared validated preferences.
2. Independent quotas with source/account provenance, explicit stale/missing/error states, relative and local absolute reset times, and provider-level categorical Pulse. Zero remains measured exhaustion. Expired resets retain the last measurement and wait for source confirmation; they never fabricate a refill.
3. Reported sessions, model/context metadata, projects with explicitly estimated—not billed—costs, and explicit agent relationships, including unresolved/cyclic relationships. No invented session ownership, agent tree, productivity score, account switch, or combined quota percentage.
4. Read-only, bounded history queries separated by provider/account/source/bucket/reset window. Accessible tables accompany independent plotted observations; missing readings remain gaps. Forecasts delegate to the conservative core model. The 500-observation prefilter limit is disclosed.
5. Native Tauri commands and capabilities scoped to the packaged dashboard, additional window/origin checks, blocking database work off the async command executor, and monotonic event revisions. Initial reads and live updates share the same sanitized cockpit facade.
6. Local deduplicated alerts, temporary silence, provider inclusion, retention, account display aliases, validated preference import/export/rollback, and explicit history-clear confirmation. Clear preserves preferences and provider-owned files. The legacy cache writer now uses the shared privacy-aware recording boundary and configured retention.
7. Keyboard tab navigation, focus handling, reduced-motion rules, narrow-layout checks, literal rendering of untrusted metadata, dirty-form preservation during live events, last-known readings on errors, and cancellation/confirmation flows.

Privacy boundaries are stated in the UI: provider inclusion affects observatory views and new recordings, not the legacy notch collectors. Disabling metadata does not erase older records. Clearing Nyrva history does not alter provider files, and subsequent collection can repopulate history.

## Actual verification evidence

### Test-first history

- Presentation RED: run `35853346117`, job `107155960864`, failed because the formatting module did not exist.
- Browser RED: run `35853706746`, job `107157132806`, failed because the dashboard entrypoint did not exist.
- Desktop facade RED: run `35854579394`, job `107159922707`, failed because the facade module did not exist.
- Browser regression found an unstable accessible name on the layout select; fixed in `d28719fe5065379d4c2181d41bc3ee5bc1006f59` without weakening the locator.

### Exact code commit `2ceed852cdf78929c8a623f3eb186b8d68e066f1`

Experience workflow: [run 35856072032](https://github.com/LSthemagic/nyrva/actions/runs/35856072032).

| Check | Job | Result |
| --- | --- | --- |
| Presentation contracts: 10 tests | `107164725302` | PASS |
| Shared desktop data facade: 6 tests | `107164725217` | PASS |
| Browser E2E of shipped HTML/modules | `107164725398` | PASS |

Browser E2E uses an injected IPC fixture, not a native desktop. It covers zero and expired reset rendering, untrusted labels, sessions/context, projects/agents, history, source health, all layouts, persisted settings, dirty-form preservation, silence, read failures, keyboard tabs, clear cancellation/confirmation, empty state, and a 640-pixel viewport. Artifact `experience-browser` contains screenshots, trace, and result JSON; presentation and bridge logs are separate artifacts. Retention is seven days; the tests and workflow remain reproducible after artifacts expire.

Native workflow: [run 35856071904](https://github.com/LSthemagic/nyrva/actions/runs/35856071904).

| Check | Linux X11 job `107164725106` | Windows job `107164724821` |
| --- | --- | --- |
| Platform sidecar preparation | PASS | PASS |
| `cargo check -p nyrva --locked` | PASS | PASS |
| `cargo test -p nyrva --locked` | PASS | PASS |
| Native smoke example compilation | PASS | PASS |
| Native webview/IPC/SQLite/UI smoke | PASS | FAIL at process startup |
| Workflow lockfile check | PASS | Not reached after smoke failure |

Linux desktop tests passed 41 unit tests plus four integration tests. The real WebKit/X11 smoke used the actual Tauri plugin, packaged dashboard modules and SQLite against an exclusively created synthetic data directory. Its seven checks passed: safe rendering, scoped native reads/revisions, preference persistence/live events, invalid-import preservation, scoped history, rejected/cancelled clear, and confirmed UI clear preserving preferences/provider sentinel. `native-smoke.json` reports `ok: true` and `provider_file_preserved: true`.

Linux evidence artifact: `experience-native-Linux`, artifact ID `10747682262` (seven-day retention). Windows evidence is retained as `experience-native-Windows`. Native smoke is an isolated test example, not a released installer; its extra reporting command is not compiled into the shipped application.

The earlier native run `35855403179` also passed compile/regression and unchanged-lockfile checks on both platforms, before the GUI smoke was added. It is not proof of Windows graphical acceptance.

## Open findings — acceptance remains blocked

### P1: Windows tray callback can block window creation

The production `nyrva/src/tray.rs` still calls `observatory::open(app)` synchronously from the menu handler. Tauri documents a Windows deadlock risk for `WebviewWindowBuilder` creation in synchronous event handlers. See the [primary API warning](https://docs.rs/tauri/latest/x86_64-pc-windows-msvc/tauri/webview/struct.WebviewWindowBuilder.html#method.new).

An attempted change to this handler was blocked by the platform's tool safety gate and **was not applied**. No alternate route was used to apply that blocked edit. The current tree therefore still contains the original synchronous call. The native smoke opens the dashboard from setup and does not cover tray-menu reentrancy, reopening, or rapid repeated clicks. These are pending production-path validation items, not passing evidence.

### P1: Windows graphical smoke failed before acceptance reporting

In job `107164724821`, the native example compiled, but the launched process exited with `-1073741511`; the smoke step failed. The cause was not established. Do not attribute this failure to WebView2 availability, a specific DLL, or the tray callback without diagnostics. The setup-based harness does not use the production tray callback, so these two findings must remain separate.

### Remaining review/validation limits

No independent reviewer subagent was available; the implementation received inline review only. No physical multi-monitor/DPI, full screen-reader session, real-provider authentication, installer installation, or release acceptance is claimed. Linux CI reported accessibility-bus/desktop-portal warnings while the explicit WebKit/IPC/SQLite assertions passed. Formatting/lint cleanliness was not separately certified; existing compiler warnings remain.

## Existing Everywhere failures: preserved, not hidden

The baseline local `cargo test -p nyrva-core --locked --no-fail-fast` and baseline core CI run `35809536011` failed four existing `everywhere_contracts` tests:

- `doctor_and_redirected_top_are_readonly_and_versioned`
- `integration_is_opt_in_and_restores_only_its_owned_field`
- `integration_refuses_to_overwrite_a_statusline_changed_after_install`
- `local_api_requires_session_auth_has_no_cors_and_rejects_writes`

Those tests and the corresponding unimplemented handlers were not changed in this Experience execution. Do not claim the entire workspace or release pipeline is green because the dedicated Experience tests pass.

## Resume without repeating completed work

Read this ledger first and use the current remote feature-branch head, not main or the older resume parent. Preserve all committed UI, facade and test work. The next work remains **closing Experience's two Windows findings and production-path validation**, not starting Everywhere. Respect any unresolved platform tool restriction; do not route around a blocked operation. Then rerun the exact affected tests and update this ledger with actual outcomes. Stop again at the macrostep boundary unless the user separately authorizes the next macrostep.

Local terminal/container execution became unavailable after baseline validation; subsequent evidence came from actual GitHub Actions jobs. No outstanding work is running on the assistant's behalf, and this ledger does not promise asynchronous completion.
