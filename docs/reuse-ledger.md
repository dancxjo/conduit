# Reuse Ledger

Treat old code as source material, not inheritance.

| Component | Source commit/file | Why retained | What was removed | New acceptance proof |
|---|---|---|---|---|
| Form lexer sketch | archive/pre-reboot-2026-08-04, parser modules | Lossless tokens and spans remain useful for S3 | Old full grammar, UI coupling, and panel-era compatibility | Pending S3 lossless-parser vectors |
| Bounded cord sketch | archive/pre-reboot-2026-08-04, hosted scheduler | Item/byte pressure and port-aware stepping are useful S1 quarry material | Production stack and broad node registry | Pending S1 multi-value pressure and terminal vectors |
| Reboot exact-plan commitments | reboot-prototype-2026-08-04, conduit-core/planner/runtime | Host, boot, generation, implementation, artifact, fragment, and evidence identities are useful | Claims that the current plan binds every required execution fact | Existing mutation negatives remain prototype evidence; S2 expands them |
| Browser/Pico/relay simulations | reboot-prototype-2026-08-04, fixtures/browser-sim and fixtures/pico-sim | Fast deterministic conformance data | Host, firmware, socket, and physical-acceptance naming | Workspace tests plus WASM/Thumb compile gates, classified only as simulation |
