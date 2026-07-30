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

## Pre-release versions and compatibility

Conduit has not made its first public release. Repository drafts do not create
backwards-compatibility obligations.

Maintain one current Conduit-owned draft per artifact family. When it changes,
migrate every repository-owned producer, consumer, fixture, example, generated
artifact, document, and snapshot; verify the migrated corpus; then delete the
displaced reader, writer, alias, migrator, hash domain, fixture, and fallback.
Git history records abandoned drafts.

Do not introduce a second accepted draft generation or a release-looking
version increment to preserve an unreleased shape. A compatibility path must
name the tagged public release that published the displaced artifact. The
exception set is empty until that release exists.

Semantic contract compatibility, exact current identities, live plan/state
transitions, and external protocol versions remain product behavior. See
[the full pre-release policy](docs/pre-release-versioning.md) and
[#191](https://github.com/dancxjo/conduit/issues/191).

## Checks

```sh
just sup
```

Without `just`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask verify-canonical
cargo check -p conduit-core --no-default-features \
  --target thumbv6m-none-eabi
```

The declared Rust 1.85 minimum is checked separately:

```sh
just msrv
```

Changes to semantic contract identity, port meaning, delivery, lifecycle,
authority, or diagnostic meaning require explicit compatibility analysis.
Before the first public release, that analysis normally results in an atomic
repository migration and deletion of the displaced draft path, not a
backwards-compatible reader.
