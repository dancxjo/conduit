# Patchbay workspace catalogue repair

Tested source: `52f76743743d72ca5a54bdb37feb4349ace759a0`, descended normally from published #6252 head `596aab0e980bb697748d44281c3ed4955f510982`. The workspace checker omitted the canonical `patchbay/accept-interaction` contract already used by native/browser owners. This repair installs its existing startup signature, checked Fore and Profile Kind; it introduces no alternate Kind, offer, or checking bypass.

The exact supported `cargo +stable xtask check plots bundle-workspace-catalog --output <path>` entrance passed and emitted 94 Plots. Two new tests passed: actual Patchbay Source and exact canonical Fore/bundle identities, plus foreign-Kind and mismatched-Fore refusal. The existing Memory Lantern semantic-editor catalogue regression passed. Workspace formatting passed. Strict all-target xtask Clippy exited 101 on inherited `owner_todo_action_proof.rs:308` (`verify_isolation` follows its tests at line358). That first failure is retained without an unrelated repair or unchanged retry.

`local-checks.json` records every command, status, clean source identity, original absolute paths, byte lengths and hashes. Gzip logs and the full generated catalogue are lossless originals; `SHA256SUMS` covers the stored packet. The preceding hosted Candidate39779112554 / browser job119826159516 negative remains in its original compressed file and `negative-custody.json`, with its distinct tested preview9ba03bb2ad6dd5fad609b8cc9e40d915174527be.

These are Source-checking/catalogue integration results. They do not establish browser rendering, Host/provider admission, CI acceptance, or completion of the other independently reported Candidate failures. No browser campaign, QEMU run, CI dispatch or PR mutation was performed.
