# Host fabrication contract

**Status:** canonical vocabulary and identity boundary for issue #1137
**Execution status:** see [STATUS.md](../STATUS.md)

Conduit has two orthogonal construction paths:

```text
SEED ── BIRTH ──> BODY
PROFILE ── BUILD ──> IMAGE
```

The words name different kinds of truth. They are not parallel aliases.

## Meaning becomes living

- **SEED** is dormant semantic material that can be opened and inspected.
- **BIRTH** is the explicit human-authorized action that creates a Body.
- the **birth Sign/event** is the bounded evidence that the action succeeded.
- **BODY** is the resulting living semantic identity, potentially realized by
  several Parts and Hosts.

`OPEN SEED` is inert. It does not admit membership, issue authority, start a
Play, or cause platform effects. `BIRTH` is explicit, attributable operator
authority; it consumes the exact opened checked Seed, creates a durable Body
identity and originating Part, and leaves the Body LULLED. BIRTH does not
implicitly Wake, Plan, or Play. Public Conduit UI, command, schema, and action
labels use **BIRTH**. Conventional past-tense prose and internal event fields
may use `born`, `birthed`, or `birth_sequence`; those name resulting state or
evidence, not a competing action.

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

## Authored Host configurations

A versioned `*.host.toml` configuration is the small structural recipe from
which the checked PROFILE is derived. It names one target, a finite set of
Bases, an explicit implementation (or finite ordered preferences) for each
Base, resource budgets, and complete finite Host limits. It contains no Form,
application pin meaning, current presence, or authority truth.

The repository entrances are:

```text
cargo xtask host configure [path]
cargo xtask host config check path/to/config.host.toml
cargo xtask host config show path/to/config.host.toml
cargo xtask host build path/to/config.host.toml
```

The configurator creates or edits the TOML source itself. Its target and Base
choices come from the same descriptors and `FabricationCatalog` metadata used
by validation and BUILD; it owns no private catalog. `check` and interactive
validation write nothing. Canonicalization sorts declaration order before
deriving the configuration identity, so equivalent structural meaning lowers
to the same existing `HostProfile` identity.

Checked examples live in `profiles/host-configurations/` for hosted Linux,
Pico W, and a browser page. BUILD manifests and IMAGE payloads retain the exact
source-configuration identity together with the resolved target, Base/driver
selections, resource budgets, and limits.

## Exact identities

The following identities never substitute for one another:

```text
SeedId                 ProfileId
source / Form identity BuildId
BodyId                 ImageId / ArtifactId
birth Sign             build receipt
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
