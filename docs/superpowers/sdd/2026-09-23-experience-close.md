# Experience completion execution

Plan: `docs/superpowers/plans/2026-09-22-nyrva-1.0.md`, Macrostep 2 only.
Spec: `docs/superpowers/specs/2026-09-22-nyrva-1.0-design.md`.
Base: merged PR #18, main `fe1e88f2517d856dee899390e51709226a6ec92d`.
Branch: `fix/experience-completion`.

Ruling: use an isolated successor branch from the confirmed merge; do not modify main or resurrect the merged branch. Cost: a follow-up PR is required.
Ruling: local container and Python both return ClientError. Use ordinary GitHub commits and real Actions verification, not an invented local worktree or test run. Cost: verification depends on runner availability.
Pre-flight: the existing dashboard consumes the shared facade; retain schema, privacy boundaries and exact native assertions. The production tray path is not currently exercised by the setup-only smoke.

Tasks:
- [ ] Reproduce Windows startup failure and inspect executable imports before changing runtime setup.
- [ ] Add production-path lifecycle coverage before fixing callback window creation and repeated opens.
- [ ] Execute Experience contracts/browser/native checks on Windows + Linux X11, review and checkpoint.

The previous platform block was on a code write, not an application bug. Any newly requested code change must use the normal connector safety checks; do not route a blocked write through another mechanism.
Everywhere and release remain outside scope; preserve/report its existing four failing contracts.
