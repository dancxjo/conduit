# Working agreement

- Preserve the separation between semantic contracts, implementations, host
  observations, execution plans, evidence, and presentation.
- `conduit-core` must remain `#![no_std]` and allocator-free. Hosted
  conveniences belong above it.
- Do not add Tongues, Netherwick, Psyched, robot, speech, model-provider, or UI
  concepts to the core.
- Every live cord is bounded. New pressure behavior requires an explicit
  semantic contract and conformance fixtures.
- `.panel` source, resolved plans, run evidence, and Patchbay presentation are
  distinct identities.
- Run focused package tests while developing. Before handoff, run formatting,
  workspace Clippy, workspace tests, and the `thumbv6m-none-eabi`
  `conduit-core` check.
- Keep commits coherent and exclude unrelated concurrent work.

## Concurrent development

- Start by inspecting `git status --short` and the relevant diff. Treat every
  unrelated change as another worker's work; do not reset, reformat, stage, or
  regenerate it.
- Claim files or hunks before editing them. If another worker is changing the
  same hunk, coordinate the handoff or wait; do not race to overwrite their
  change.
- Stage explicit paths only. Before publishing, fetch `origin`, rebase onto
  the current `origin/main`, and rerun the focused checks for the resulting
  tip. Never force-push or rewrite another worker's branch.
- A commit is the cross-system handoff. Keep it small, name the behavior it
  changes, and include only the files needed for that behavior so independent
  workers can rebase and integrate it safely.
- Diagnose CI at the failed commit's exact tip. Do not update a fixture, a
  generated artifact, or a snapshot solely to silence an assertion; regenerate
  one only when its producer's intended current semantics changed and its
  authoritative verification requires it.
- Tour browser tests assert selectable behavior, run results, and visible
  failures. They do not mirror plan hashes, artifact digests, scenario or
  documentation inventory counts, source spelling, or timeline indices. Keep
  exact semantic identity checks in their owning Rust/conformance boundary.

## Pre-release compatibility rule

Conduit has not made its first public release. Do not preserve backwards
compatibility for repository drafts.

When changing a Conduit-owned grammar, schema, manifest, plan, protocol,
catalog, evidence shape, Tour resource, or generated artifact:

1. maintain exactly one current pre-release form;
2. migrate every repository-owned producer, consumer, fixture, example,
   embedded source, generated asset, document, and snapshot;
3. verify the migrated corpus;
4. delete the displaced parser, reader, writer, alias, migrator, hash domain,
   fixture, and fallback before completing the work.

Git history is the archive. Do not call an unreleased form `legacy`, `stable`,
or `frozen`. Do not add `v2` merely to avoid replacing an unreleased `v1`.
Do not retain a one-time migrator after its repository migration has run.

A compatibility path is allowed only when it names the tagged public Conduit
release that published the displaced artifact. Before the first release, that
exception set is empty.

This rule does not remove semantic contract compatibility, exact current
artifact identity, live plan/state transitions, or external protocol versions.
Read [the pre-release version policy](docs/pre-release-versioning.md) and
[#191](https://github.com/dancxjo/conduit/issues/191) before changing versioned
or persisted surfaces.
