# Experience execution ledger

Plan: `docs/superpowers/plans/2026-09-22-nyrva-1.0.md`.
Spec: `docs/superpowers/specs/2026-09-22-nyrva-1.0-design.md`.
Resume parent: `e7d4ea6ea72dc6c9f621481098d4ee3bce6a9e28` on `feat/nyrva-1.0`.

## Scope

Only Macrostep 2 (Experience). Preserve the notch and provider adapters. No merge, tag, release, forced ref update or implementation of Everywhere handlers.

## Baseline evidence

Core CI run `35809536011` and an offline local rerun of `cargo test -p nyrva-core --locked --no-fail-fast` have four existing failures in `everywhere_contracts`: redirected top, reversible integration installation, changed integration refusal, and the authenticated local API. The other core/cockpit contracts pass. These four tests remain enabled and are not acceptance evidence for Experience.

## Pre-flight and rulings

- Initial reads and live events must use the same sanitized cockpit contract. The UI may format, but must not invent quota, session relationships, forecasts or billing.
- Settings use the existing validated control plane, not an independent localStorage preference copy. Clearing is limited to Nyrva history and alerts.
- Ruling: resume from the feature branch, not main; this preserves the newer session work. Cost if wrong: changes remain isolated on this branch.
- Ruling: use a separate focusable dashboard instead of changing the no-activate notch window. This preserves its placement, drag and DPI behavior. Cost: dashboard and notch are separate windows.
- Ruling: the terminal became unavailable after the successful baseline execution. Continue through GitHub commits and actual CI logs; never mark unexecuted tests passed. Cost: native acceptance can remain explicitly blocked.

## Tasks

1. Presentation contracts before implementation: committed; RED execution pending.
2. Dashboard, layouts, reset timeline, history, sessions, projects and agents: pending.
3. Desktop bridge, tray, live events, alerts, settings and Privacy Center: pending.
4. Browser E2E, Windows/Linux compilation, review and final checkpoint: pending.

Release 1.0 remains blocked. The next macrostep is not started by this ledger.
