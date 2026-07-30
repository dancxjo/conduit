# Changelog

Conduit is pre-release software. Entries describe only claims admitted by
`release/capabilities-v1.json`; a closed issue or merged commit is not itself a
release claim.

## 0.1.0 — unreleased

- Added the canonical exact-plan production path for hosted CLI and browser
  execution with bounded scheduler evidence.
- Added the executable `literal -> uppercase -> stdout` vertical proof.
- Replaced Patchbay source inference with Rust-produced, revisioned
  projections and typed edits.
- Classified every checked-in panel as runnable, contract-only, or
  illustrative/unavailable.
- Added a bounded one-request Linux loopback HTTP provider and deterministic,
  Linux TCP, and rustls conformance.

Exact commit, claims digest, runnability digest, supported-host boundary,
license, and evidence paths are emitted by:

```sh
cargo xtask release-gate --check --output target/release-evidence.json
```

The command refuses to emit release evidence from a dirty worktree.
