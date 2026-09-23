# SDD ledger — plan: docs/superpowers/plans/2026-09-23-everywhere.md

Base main: 88f25596d3b0e3c5f4f7fd76d7dab7d7f36a89a5. Remote implementation branch: feat/everywhere; source checkpoint 93a0baeab7da8037aaff0049b4d5d9df37f8aab6. Local archive tree exactly matches af0aa19d30ffa69be4c71eed1720f06abfc8777a. Fresh isolated local workspace; no user checkout is modified.

Baseline: local `cargo test -p nyrva-core --offline --locked --no-fail-fast` fails only the four existing Everywhere contracts. Core/Experience work is preserved. Native compiler and registry recovered from the prior verified CI kits; current network cloning is unavailable.

Pre-flight: terminal/API consume existing sanitized views; integration-generated commands must pass an explicit data root to the same ingestion boundary. Maintenance trust and integration receipts must not enter portable preferences or diagnostic export.

Ruling: implement the already-approved macrostep inline without another approval round; no subagent execution tool is available. Cost: review is author self-review rather than independent review.
Ruling: signed updates are explicit trust configuration and cryptographic local-artifact verification, never automatic execution without publisher trust and release acceptance. Cost: installation remains an explicit user action, as the spec's fail-closed release rule requires.

Tasks 1–6: in progress. No completion or release claim.

Core implementation verification: `cargo test -p nyrva-core --offline --locked --no-fail-fast` → 68 passed, 0 failed, 0 ignored. The previous four Everywhere failures now pass. Extended test-first coverage includes terminal redirect, generated shell command execution, duplicate JSON/symlink refusal, authenticated HTTP/SSE, session rotation, signed artifact tampering, and lossless migration rollback.

Terminal finding: standalone entrypoint held stdin's lock across the interactive loop, starving its keyboard worker. A real PTY reproduced failure to exit with q+Enter; removing that outer lock made the same probe exit successfully. Desktop dispatch test first failed because doctor still selected legacy diagnostics; the shared-command routing now passes locally. Native linking initially required correcting this container's extracted OpenSSL lookup; that environment adjustment is not a product patch.

Recovery finding: interrupted integration removal could leave a receipt after successful restoration. A failing regression now passes: cleanup removes only the matching receipt, never rewrites the already-restored provider file. Same-timestamp independent sessions block lossy database rollback.

Remote acceptance, packaging and final review remain pending. This is not a completion claim.
