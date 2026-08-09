# Reboot Kernel M0 Sign

This records the local validation for GitHub issue #349,
`[Milestone M0] Reboot kernel exactness and adapter readiness`.

## Issue Contract

- Exact issue: https://github.com/dancxjo/conduit/issues/349
- Re-fetched: 2026-08-05T04:16Z
- Base `HEAD` at validation: `c13478e01ddabc2ec5a7e6bb73982fb2a74e8a51`
- Scope: M0 kernel exactness and adapter readiness. Later milestone issues
  #350-#360 are treated as downstream constraints, not completion targets here.

## Validation

The following commands passed against this working tree:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Supplemental embedded-sensitive check:

```text
cargo check -p conduit-core --target thumbv6m-none-eabi
```

## Acceptance Mapping

- Runtime remains semantic-profile independent: executable semantics are
  installed through runtime implementation registries rather than a
  `conduit-signal` production dependency or kind-name match in runtime.
- Plan and fragment identities bind executable fields including implementation
  IDs, artifact IDs, ports, value kinds, connection base and capacities,
  startup order, expected terminals, and expected Sign.
- Runtime preparation verifies sealed plan identity before accepting work and
  rejects mutated executable identity fields deterministically.
- Composite capability boundaries are derived from checked form exports and the
  exact internal plan boundary.
- Std host output is streamed through a writer path and signal pulse counts are
  bounded.
