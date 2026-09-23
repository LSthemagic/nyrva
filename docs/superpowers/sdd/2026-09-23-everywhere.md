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

## Resume 2026-09-23 — completion pass

Recovered remote HEAD `6789699464f5bf76b4b1c0773e4617b38e518164` from Actions artifact `10755377715`; the isolated local Git tree exactly matched `0eeefa641e4eba9e55fb1c66f55cbded8794fc94`. All 15 commits above main were retained. No user checkout, ref history, version or release request was overwritten.

CI run `35871910672` had ten successful jobs and one failure: Windows distribution acceptance read a rejected POST response until TCP EOF and encountered WinError 10053. New real-socket tests first reproduced the unnecessary EOF wait and acceptance of truncated bodies (RED); a bounded Content-Length reader now passes them (GREEN), while preserving all 401/403/405 and JSON checks. Incomplete bodies, duplicates, transfer encoding and oversized responses still fail. No socket exception is suppressed.

Verification before push: Python tooling 36/36 passed; extracted Linux packaged CLI completed all seven black-box groups including real shell, sockets, migration and PTY. Production Rust code was unchanged. Local staged tree `db5871e7e814d8b63ff8f72050e4e306ec312d56` matched the created remote tree byte-for-byte. Commit `4a5543dfd23f6ca3234b49cda9e187fb3aa4641d` was fast-forwarded to `feat/everywhere`; acceptance run `35876115970` is the current gate, not yet claimed green.

User ruling: physical native acceptance on their own Windows/Linux machine is performed by the user. Automated native CI, package verification and implementation closure remain this task's responsibility. Final release publication still requires the documented manual acceptance and separate maintainer action.


The first correction addressed HTTP framing but did not fix the server race: run `35876115970` failed on an authenticated GET before its response headers. Investigation found that Winsock inherits the nonblocking listener mode for accepted sockets; the per-client timeouts do not turn blocking I/O back on. Unix control probes waited correctly for both a delayed first byte and segmented headers.

Test-first commit `64919eb6f23bcd3b638caa9de0054e5e7025179c` adds a real subprocess/TCP regression. Run `35876635274`, artifact `10759000389`, reproduced RED on Windows: `API answered or closed before a complete request and before the 2s deadline: Ok(1)`, in 0.04 seconds. Linux's same suite passed. No authentication, response-code or timing assertion was weakened.

Commit `712c94615c05b330609c12abbbeae5e7376a2bbc` explicitly resets each accepted socket to blocking mode before bounded worker I/O (also before a busy response); the listener stays nonblocking, limits remain unchanged, and failed mode configuration closes the connection. Its local tree exactly matches `2983a1eef27cd325fd447bcd3ddc33fafa33d00f`. Automated acceptance is tracked by run `35877368334`; completion is gated on the actual results, not the hypothesis.


GREEN evidence: run `35877368334` core artifacts report 70 Windows and 69 Linux tests, zero failures/ignored/filtered tests. Both native Experience jobs and presentation/browser regression passed. Linux distribution produced artifact `10758997203`; artifact `10759582054` records installed console/desktop, extracted AppImage console and external AppImage doctor success. Both the new Debian console and external AppImage subsequently passed all seven black-box groups in this container. Windows desktop/standalone black-box acceptance passed before the packaging toolchain step; the installed Windows package remains a gate until observed.

Documentation review corrected a non-existent `history --provider` flag to the actual positional provider syntax, verified 14 documented read commands in an isolated data root, and retained all manual release gates. Python 36/36 and presentation 10/10 passed again after the documentation edits. Physical acceptance, release publication and independent review have not been claimed.


Run `35877368334` subsequently built and silently installed NSIS, but failed the installed-console ACL probe; installed-desktop acceptance was not reached. The debug smoke was hosted by Git Bash, while package acceptance was hosted by PowerShell 7. Windows PowerShell 5.1 inherited PowerShell 7's incompatible module search path through Python. The next correction preserves the protected-DACL/single-rule assertion and 5-second server duration; only the ACL child process environment drops PSModulePath and now reports bounded, non-secret errors.

Commit `7c2a6b301302e1a525825043e0d37feef8d3da76` includes two environment regressions (observed RED then GREEN), 38/38 local Python tests and another complete packaged Linux-console smoke. The native Windows core job in run `35881107675` passed both all Rust contracts and the newly added PowerShell-parent full CLI smoke. Source tree was verified as `1b872ad013d8d39b5a04108467ed2c3a8a6addb2` before the fast-forward update.

Tasks 1–6 have their implementation, executable contracts and documentation in the branch. Automated closure is defined by all eleven jobs and installed-package reports in the final PR's referenced CI run, not by a presumed percentage. Documentation-only changes do not change the executable source above. `docs/everywhere-acceptance.md` maps the scope, reproduction commands and preserved evidence. The maintainer's physical S01–S20 acceptance, main integration, version selection and public release remain separate actions; none is falsely marked as executed here.
