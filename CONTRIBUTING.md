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

## Concurrent changes and checks

Several contributors and systems may land work at once. Inspect the working
tree before editing, leave unrelated changes untouched, and stage only the
paths that belong to your change. Keep commits narrow enough to rebase without
pulling in another change's cleanup. Fetch and rebase onto current `main`
immediately before publishing; resolve an overlapping hunk with its owner
instead of racing or rewriting their branch.

When sharing one checkout, claim a file or hunk before editing it and reread
`git status --short` before every generated command or commit. Do not reset,
format, regenerate, stage, or delete another worker's changes. When working
from separate machines, a small commit is the handoff: fetch, rebase the
commit onto the current `origin/main`, rerun its focused evidence at that tip,
and push without force. If two changes overlap, stop at the overlap and agree
on one combined owner; do not solve a race by replaying a stale fixture or
rewriting the other branch.

Use the narrowest test that proves the behavior being changed. In particular,
Tour browser tests cover scenario selection, admitted/rejected outcomes, and
observable execution. They must not duplicate plan hashes, artifact digests,
generated inventory counts, static source spellings, or timeline positions.
Those facts belong to their exact compiler, artifact, or conformance checks.

Do not refresh a fixture, generated artifact, or snapshot merely to restore a
failing assertion. Refresh it only when the intended current producer changed
and the owning verification demonstrates that the new artifact is required.
Plan identities, artifact digests, and generated browser payloads are producer
outputs, not browser-test expectations. Keep presentation tests focused on
selection, visible state, and admitted outcomes; diagnose an identity drift at
the owning contract, compiler, or artifact boundary first.

Concurrent browser runs must use distinct ports, for example
`CONDUIT_PLAYWRIGHT_PORT=4194 npx playwright test ...`. The default local
Playwright server is reusable for one checkout; sharing that port across
worktrees can silently serve another checkout's Tour resources.

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
