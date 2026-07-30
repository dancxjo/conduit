# Pre-release version and compatibility policy

Conduit has not made its first public release. Until that happens, repository
drafts are not compatibility commitments.

## The rule

Each Conduit-owned artifact family has exactly one current pre-release form.
When that form changes, migrate every repository-owned producer, consumer,
fixture, example, generated artifact, document, and snapshot in the same
integration change. Once the migrated corpus passes, delete the displaced
parser, reader, writer, alias, migrator, semantic-hash domain, fixture, and
fallback path.

Git history records abandoned drafts. Production code does not preserve them.

Do not call unreleased drafts `legacy`, `stable`, or `frozen`. Do not add a
compatibility path merely because a prior commit wrote a different shape.

## Current pre-release scheme

Before the first tagged release:

- the workspace package version is development metadata, not a support promise;
- each Conduit-owned source, serialized, persisted, or wire artifact uses one
  current draft marker, consistently represented as schema version `0` or a
  `/draft` identifier within that artifact family;
- each family has one current encoder and one current decoder;
- candidate specifications and conformance suites use topical names rather
  than accumulating release-looking `-v1`, `-v2`, and similar suffixes;
- draft changes replace the current definition and regenerate repository-owned
  artifacts rather than adding a second accepted generation;
- the exception ledger for released compatibility obligations is empty.

At the first public release, establish the initial supported baseline
deliberately. Only a tagged public release can create a Conduit-owned
backwards-compatibility obligation. A later compatibility reader or migration
must name the release that published the displaced artifact.

## What this policy does not remove

These are product semantics and remain:

- TypeContract and PortContract compatibility and substitution proofs;
- exact identities used to compare current artifacts;
- explicit live plan/state transition and handoff contracts;
- versions of external standards and protocols;
- compatibility for artifacts actually published by a tagged Conduit release.

A module named `compatibility` is not automatically draft archaeology.
Classify behavior by purpose before removing it.

## Changing a draft

A draft-breaking change must land atomically:

1. choose the one new current shape;
2. update all repository-owned writers and readers;
3. migrate source, fixtures, examples, snapshots, generated assets, exact
   plans, catalogs, Tour content, WASM/browser plans, and documentation;
4. run the complete checks against the migrated repository;
5. delete the one-time migrator and every displaced draft path;
6. add or update a gate proving that only the current draft is accepted.

Do not merge a temporary two-schema production state. If the work is too large
for one review, preparatory commits may exist on one branch, but main remains
current-only.

## Agent instructions

Before introducing a version number, compatibility reader, migration, alias,
deprecated form, or fallback:

1. identify the tagged Conduit release that created the obligation;
2. if no tagged release did, update the current draft in place;
3. migrate repository-owned artifacts;
4. delete the displaced draft machinery before completion.

An issue number, specification number, commit hash, generated fixture, or old
main-branch state is not a release.

## Enforcement

The repository gate introduced by
[#191](https://github.com/dancxjo/conduit/issues/191) must reject:

- multiple accepted Conduit-owned draft generations in one family;
- migrations, aliases, deprecated readers, or old-draft fixtures without a
  tagged-release entry;
- canonical source or generated assets using a displaced draft;
- release-looking version increments used to preserve unreleased history;
- prose that treats an unreleased draft as a compatibility commitment.

Exceptions list public release tags and the exact published artifact family.
Before the first release, the exception list is empty.
