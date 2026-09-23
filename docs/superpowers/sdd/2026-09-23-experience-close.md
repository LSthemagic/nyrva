# Experience completion execution

Plan: `docs/superpowers/plans/2026-09-22-nyrva-1.0.md`, Macrostep 2 only.
Spec: `docs/superpowers/specs/2026-09-22-nyrva-1.0-design.md`.
Base: merged PR #18, main `fe1e88f2517d856dee899390e51709226a6ec92d`.
Branch: `fix/experience-completion`.

Ruling: use an isolated successor branch from the confirmed merge; do not modify main or resurrect the merged branch. Cost: a follow-up PR is required.
Ruling: local container and Python both return ClientError. Use ordinary GitHub commits and real Actions verification, not an invented local worktree or test run. Cost: verification depends on runner availability.
Pre-flight: the dashboard consumes the shared facade; retain schema, privacy boundaries and exact native assertions. The old setup-only smoke did not exercise tray event-loop behavior.

## Task 1: Windows native process startup

RED baseline run `35858633715`, Windows job `107173097980`: desktop compile and 27 desktop tests passed; native example exited `-1073741511` before writing its synthetic directory/report. Initial import inspection inside Python used Python's activation context and was insufficient to identify the standalone executable's selected Common Controls version.

RED diagnostic run `35859515310`, Windows job `107175988186`: the example had `embedded_manifests: []`, `common_controls_v6: false`, and imported `TaskDialogIndirect`, absent from System32 comctl32.dll. `scripts/experience-loader.py` failed the explicit manifest assertion. The example must declare the Common Controls v6 dependency just like the application.

Fix: add a test-example-only embedded manifest through Cargo's example linker arguments on Windows MSVC. It requests `asInvoker`, not elevation. No runtime downloads, DLL replacement or PATH manipulation. Application resources and permissions remain unchanged. GREEN execution pending.

## Task 2: Production window lifecycle

Tests committed before production changes. The native example dispatches the actual production opener from the event-loop thread after setup, requires that callback to return, and exercises close/hide/reopen plus 32 rapid requests reusing one window. The new Linux lifecycle test also failed in run `35859515310`; inspect its exact cause before changing the opener. Windows runtime test is gated on the missing manifest fix above.

## Task 3: Completion verification

Presentation and browser passed in run `35859515310`; facade tests passed on both platforms. Full core regression remains separate and unfiltered: known Everywhere failures are not converted to acceptance success. Final exact-head verification and review remain pending.

The prior tool block was on a code write, not an application error. Any new write uses normal connector safety checks; no blocked action is routed through another mechanism. Everywhere and release are outside this task.
