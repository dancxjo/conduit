# Functional compatibility: the face is the contract

**Status:** canonical architecture direction  
**Applies to:** Forms, catalog Kinds, Host offers, planning, reusable composition, and future shared pools
**Related:** #507, #511, #512, #514, #515

## Rule

Conduit uses **functional compatibility**, not nominal compatibility.

> **Two callable Conduit things are compatible when their canonical checked faces are equal.**

A catalog path, Form name, Kind ID, Gear ID, implementation name, artifact identity, or revision label does not by itself make two things compatible or incompatible.

Names remain valuable for authorship, discovery, catalog organization, provenance, diagnostics, Sign, and exact realization records. They are not hidden nominal types.

For the first implementation, compatibility is deliberately simple:

```text
same canonical checked face     -> compatible
different canonical checked face -> incompatible
```

This is exact equality, not a width/depth/variance subtyping lattice.

## What belongs to the face

The checked face is the complete public callable boundary Conduit has admitted for the Form or Kind. Whatever the checked face model contains participates in compatibility.

At minimum, the current language direction includes:

```text
startup parameter signature
    names
    positional order
    value types
    required/default shape

runtime ports
    names
    direction
    value type
    temporal shape

shorthand path
    the declared input -> output path, if any
```

The back does not participate in compatibility. Two forms may have radically different backs and remain compatible if their checked faces are equal.

If an observable semantic distinction must prevent substitution, that distinction must be represented in the checked face contract. It may not be hidden behind a friendly name and then enforced nominally.

## Forms and Kinds share the same compatibility law

A reusable Form and a Host-offered primitive Kind are not separate compatibility universes.

Conceptually:

```conduit
form loud (
    text: Text > text: Text
) {
    upper: text/upper
    text > upper > text
}
```

If another callable thing has the same checked face as `loud`, it is compatible with `loud` at that boundary regardless of whether it is:

- another reusable form;
- a standard catalog Kind;
- a Host-native implementation exposed through a Kind offer;
- a browser/WASM realization;
- a bounded embedded realization.

The planner may therefore choose among face-compatible realizations without requiring their catalog/form names to match.

## Planning

Planning separates **compatibility** from **exact realization**.

Candidate admission begins with face compatibility:

```text
gear's required checked face
        ↓
face-compatible host/form realizations
        ↓
resource + authority + observation + policy filtering
        ↓
selected exact realization
        ↓
immutable Plan
```

Once a realization is selected, the Plan remains exact. It may seal:

- exact host and boot as appropriate;
- exact implementation and artifact identity;
- resources and reservations;
- authority;
- connections and route candidates;
- finite limits;
- Sign requirements.

Functional compatibility therefore does **not** mean runtime improvisation. A compatible realization absent from an already-sealed Plan cannot be substituted opportunistically unless the Plan explicitly admitted that alternative or a new planning pass produces a new Plan.

## Names and revisions

Names and revisions are provenance and catalog facts, not compatibility gates.

Therefore:

```text
same face + different name       -> compatible
same face + different revision   -> compatible
different face + same name       -> incompatible
different face + same revision   -> incompatible
```

A revision change that changes the checked face is naturally incompatible because the face changed. A revision change that leaves the canonical checked face unchanged does not create incompatibility merely by changing the revision token.

Proof and conformance Sign remain attached to the exact implementation/artifact/revision that was actually tested. Functional compatibility does not transfer historical proof claims to an untested implementation.

## Identity

Keep these identities separate:

```text
source/form/catalog identity
checked face identity
expanded form identity
selected implementation/artifact identity
Plan identity
Play identity
Sign identity
```

`FaceId` or an equivalent canonical checked-face digest may be useful internally. The exact representation is an implementation choice, but compatibility must derive from the checked face rather than from the source/catalog name.

Two differently named things with the same checked face may have different source/catalog identities while sharing the same compatibility class.

## Cords

Cord compatibility follows the same functional principle at the connected boundary. Value type, direction, temporal behavior, bounds, and other checked port facts must agree as required by the face contract.

Do not infer compatibility from declaration order, friendly names alone, or implementation technology.

## Catalogs and host families

Catalog categories such as `text/`, `time/`, `flow/`, `web/`, or `llm/` remain useful organization and opt-in packaging boundaries.

A Host may advertise named Kinds for discovery and Signs, but planning eligibility is based on their checked faces plus other explicit planning requirements. Category prefixes and Kind names do not form a nominal type hierarchy.

A host compiled with an opt-in family still advertises only the exact realizations it can currently promise. Functional compatibility does not weaken runtime truth or finite limits.

## Shared pools

A shared pool's member contract is likewise a checked face. A pool may admit members that are functionally compatible with the pool's declared member face even if those members come from differently named Forms or Host-provided Kinds.

Pool identity, member identity, membership epochs, authority, and finite capacity remain exact runtime/Plan facts. Face compatibility does not make pools ambient or unbounded.

## Diagnostics

Prefer diagnostics such as:

```text
face mismatch
missing startup parameter
runtime port mismatch
temporal shape mismatch
shorthand mismatch
no face-compatible realization
```

over nominal errors such as:

```text
wrong Kind name
wrong Kind ID
wrong catalog path
wrong revision
```

A name/revision may still appear in a diagnostic to identify the candidate being discussed, but it must not be the reason for incompatibility when the faces are equal.

## Migration from the nominal checkpoint

PRs #520 and #521 intentionally implemented the then-current nominal rule. That rule is now superseded.

Follow-up work must remove or invert tests asserting that:

- a differently named form with the same face is incompatible;
- an offer with the same face but a different Kind identity is ineligible;
- a revision difference alone makes a candidate incompatible;
- structural/face coincidence must be rejected.

Replace them with positive and negative proofs:

1. differently named callables with exactly equal checked faces are compatible;
2. a same-named callable with a changed face is incompatible;
3. planning can choose a differently named face-compatible host offer and still seal its exact implementation/artifact identity;
4. changing only the selected exact realization changes Plan identity as appropriate without changing face compatibility;
5. incompatible startup/runtime/temporal/shorthand faces fail closed.

## Non-goals

This rule does not introduce:

- implicit coercions;
- width/depth structural subtyping;
- variance rules;
- duck-typed runtime dispatch;
- ambient dynamic plugin selection;
- unplanned runtime substitution;
- proof transfer between implementations;
- weakening of resource, authority, transport, or Sign exactness.

## Canonical sentence

> **The face is the contract. If the face is the same, it fits. The Plan still records exactly what was chosen.**
