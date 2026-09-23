# Experience checkpoint — superseded by completion

**Current checkpoint: `docs/superpowers/sdd/2026-09-23-experience-close.md`.**

The two Windows blockers recorded earlier on 2026-09-23 are now closed on `fix/experience-completion`, verified at code commit `fbe86ec61e87cdb1074a2837492e3f98153344cc` in Actions run `35861118995`.

The native smoke startup failure was caused by the Cargo test example's missing Common Controls v6 manifest. The production opening entrypoint now defers window creation outside the synchronous event callback and coalesces repeated requests. Native WebKit/X11 and Windows WebView2 acceptance both passed, including hide/reopen and preservation of the notch and provider-file sentinel.

The earlier implementation and diagnostic history remains available in this file at commit `36c7c44af7f691a8739a48973f3793da4dd2538c`, merged by PR #18. It must not be treated as the current task state.

**Macrostep 2 is complete within its scoped automated acceptance. Everywhere and release 1.0 are not complete.** The four pre-existing Everywhere contracts remain failing in the unfiltered full-core suite. Read the completion ledger for exact evidence, scope and release-validation exclusions.
