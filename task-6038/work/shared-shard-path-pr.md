The delegated native feature-coverage test added in #6148 compiles in the full xtask binary but fails in the dependency-light dispatcher, where its source module has a different parent. This breaks dev lint/products and the held #6055 Candidate before native product acceptance.

Use the existing `crate::suites` re-export shared by both binaries for the helper and package constant. The feature inventory and delegated native proof selection remain unchanged.

Validation passed: strict Clippy across all targets of both tooling packages; both actual Cargo feature-inventory tests through each binary entrance (four passes); supported `cargo +stable xtask ci pipeline preflight` (288 tests plus formatting/standalone-lock checks).

This repairs shared CI admission for #5994 and #6038; neither parent is complete.
