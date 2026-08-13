# Host fabrication contract

**Status:** canonical vocabulary and identity boundary for issue #1137
**Execution status:** see [STATUS.md](../STATUS.md)

Conduit has two orthogonal construction paths:

```text
SEED ── BORN ──> BODY
PROFILE ── BUILD ──> IMAGE
```

The words name different kinds of truth. They are not parallel aliases.

## Meaning becomes living

- **SEED** is dormant semantic material that can be opened and inspected.
- **BORN** is the explicit Body transition and its bounded Sign.
- **BODY** is the resulting living semantic identity, potentially realized by
  several Parts and Hosts.

`OPEN SEED` is inert. It does not admit membership, issue authority, start a
Play, or cause platform effects. `BE BORN` is the explicit transition. Public
Conduit UI, command, schema, and evidence labels use **BORN**, not `birth`, for
that transition. Conventional internal Rust method and field names may use
`born` or `birth_sequence` where grammar requires them; they do not define a
competing public noun.

## Machinery becomes concrete

- **PROFILE** is a finite declarative description of one Host to construct.
- **BUILD** is deterministic resolution, validation, and fabrication of that
  PROFILE under exact source and toolchain inputs.
- **IMAGE** is the exact BUILD output plus its bound machine-readable manifest.

An IMAGE is machinery, not meaning. After BUILD it may be **FLASHed**,
**LOADed**, **LAUNCHed**, or **BOOTed**, according to its target. Conduit does
not introduce a generic installed-IMAGE state.

`OPEN PROFILE` is also inert: reading or validating construction intent creates
no Host, Boot, offer, Body, Plan, Play, membership, or authority. BUILD emits a
build receipt and IMAGE evidence; it never emits a Born Sign.

## Exact identities

The following identities never substitute for one another:

```text
SeedId                 ProfileId
source / Form identity BuildId
BodyId                 ImageId / ArtifactId
Born Sign              build receipt
                       HostId
                       BootId
                       OfferGeneration
```

A rebuild creates fresh BUILD and IMAGE truth. A launch or boot creates fresh
Boot and offer-generation truth. A durable HostId may remain the same only
under the existing Host identity contract. None of those changes rewrites an
old immutable Plan or changes a Body identity.

Compiled capability is possibility, not current availability. A running Host
may offer only the subset of its IMAGE whose exact runtime prerequisites are
currently satisfied and authorized.

## Bootable target identity

For a target whose final load unit is a packaged artifact, the resolved
`HostImage` JSON is a BUILD description, not a second final IMAGE. The checked
PROFILE and its resolved dependency closure enter the target lowerer, which
compiles the freestanding kernel and packages that description with the pinned
boot assets. The resulting target receipt keeps these identities distinct:

```text
ProfileId
  -> BuildId
  -> resolved-description binding
  -> kernel artifact digest
  -> pinned boot-asset provenance
  -> final ImageId = digest(final bootable bytes)
```

The final digest is recorded outside the bytes it hashes, so the relationship
is non-circular. `resolved_description_binding` is deliberately not named an
`ImageId`: it proves which finite resolved BUILD description entered the
package. For ConduitOS x86_64, `cargo xtask host build` owns this full path and
`cargo xtask host verify` recomputes both artifact digests and the description,
PROFILE, BUILD, and target bindings. The lower-level `cargo xtask conduitos
image` command remains target-development machinery and emits no competing
canonical Host-fabrication identity.

BUILD still creates no Host, Boot, offer, Body, Plan, or Play. Carrying the
bounded description as a boot asset establishes artifact provenance only;
validating it inside a fresh Boot and deriving current offers remain separate
runtime obligations.

## Public schema vocabulary

New Host-fabrication schemas and APIs spell the public concepts `PROFILE`,
`BUILD`, and `IMAGE`, with conventional expanded type names such as
`HostProfile`, `BuildManifest`, and `HostImage`. They do not introduce `PROF`,
`MAKE`, or `IMAG` aliases. No compatibility alias is provided for pre-v0
project-owned lifecycle labels.

This note freezes the vocabulary, not a package manager, installer, dynamic
module system, or Body lifecycle redesign.
