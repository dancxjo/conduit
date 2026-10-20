Parent #5992; discovered while validating #6165.

The supported headless Host build succeeds, but `cargo xtask make conduitos headless-proof <output>` is unrecognized: existing headless_proof.rs/validation.rs are not registered in the xtask command topology. The retained final ELF is also stripped, so the existing verifier cannot establish its required entry/symbol exclusions. No headless guest or symbol-exclusion acceptance has been claimed.

Restore the smallest supported proof route and ensure the exact final headless artifact remains inspectable, with artifact hashes retained. Preserve graphical build behavior and finite admission; do not weaken the verifier into accepting unavailable symbols. Keep command composition thin and substantive proof in its existing responsibility modules.

Acceptance: registered required-output CLI, focused negative validator/CLI tests, final headless ELF entry and graphical/parser-symbol exclusions, absence of every pinned TTF/OTF asset, two real unsupported-startup boots with exact profile/build/image binding and zero initialized providers/workload. Record the honest unsupported boundary and actual checks. Link the repair and refreshed source-pinned evidence into #5992/#6165.
