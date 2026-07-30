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

Before the first tagged release, use this matrix:

| Surface | Current pre-release representation |
| --- | --- |
| Workspace/package SemVer | One development-only version: `0.0.0-dev` |
| Numeric schema or grammar field | `0` |
| Panel source header | `panel 0` |
| Conduit-owned contract/package/interface name | Unversioned canonical name |
| Exact artifact identity | Canonical content hash, separate from the name |
| Spec, fixture, and conformance filename | Topical name with no `-v1`, `-v2`, or generation suffix |
| External protocol or standard | Its real external version |
| Released-compatibility exception | None |

A family has one current encoder and one current decoder. Draft changes replace
the current definition and regenerate repository-owned artifacts; they do not
add a second accepted generation. Do not add `@1`, `/v1`, `V2`, `plan-v9`, or a
similar release-looking marker to distinguish one unreleased Conduit draft from
another. When an immutable current artifact must be compared exactly, use its
canonical content identity, not a fake release number.

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
