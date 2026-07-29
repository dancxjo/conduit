# Contributing

Conduit is small at the center by design. Before adding a core concept, identify
the motivating domain-neutral requirement and the conformance fixture that
would fail without it.

## Before opening a change

- Decide whether the change belongs to semantic contracts, an implementation,
  host observation, source authoring, execution, evidence, or presentation.
- Keep domain concepts in their domain profile.
- Preserve `conduit-core` as allocator-free `no_std`.
- Make every live buffer finite.
- Give rejectable behavior a stable diagnostic.
- Add both a positive and negative fixture when possible.

## Checks

```sh
just sup
```

Without `just`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 conformance/c1/verify_canonical_v1.py
cargo check -p conduit-core --no-default-features \
  --target thumbv6m-none-eabi
```

The declared Rust 1.85 minimum is checked separately:

```sh
just msrv
```

Changes to stable contract identity, port meaning, delivery, lifecycle,
authority, or diagnostic meaning require explicit compatibility analysis.
