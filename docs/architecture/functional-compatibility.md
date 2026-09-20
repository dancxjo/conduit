# Functional compatibility: the front is the contract

**Status:** canonical architecture direction  
**Applies to:** forms, catalog kinds, host offers, planning, reusable composition, and shared pools
**Related:** #507, #511, #512, #514, #515

## Rule

Conduit uses **functional compatibility**, not nominal compatibility.

> **Two callable Conduit things are compatible when their canonical checked fronts are equal.**

A catalog path, form name, kind ID, gear ID, implementation name, artifact identity, or revision label does not by itself make two things compatible or incompatible.

Names remain valuable for authorship, discovery, catalog organization, provenance, diagnostics, sign, and exact realization records. They are not hidden nominal types.

Compatibility uses exact checked equality:

```text
same canonical checked front     -> compatible
different canonical checked front -> incompatible
```

This is exact equality, not a width/depth/variance subtyping lattice.

## What belongs to the front

The checked front is the complete public callable boundary Conduit has admitted for the form or kind. Whatever the checked front model contains participates in compatibility.

The public boundary includes:

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

The back does not participate in compatibility. Two forms may have radically different backs and remain compatible if their checked fronts are equal.

If an observable semantic distinction must prevent substitution, that distinction must be represented in the checked front contract. It may not be hidden behind a friendly name and then enforced nominally.

## forms and kinds share the same compatibility law

A reusable form and a host-offered primitive kind are not separate compatibility universes.

Conceptually:

```conduit
form loud (
    text: Text > text: Text
) {
    upper: text/upper
    text > upper > text
}
```

If another callable thing has the same checked front as `loud`, it is compatible with `loud` at that boundary regardless of whether it is:

- another reusable form;
- a standard catalog kind;
- a host-native implementation exposed through a kind offer;
- a browser/WASM realization;
- a bounded embedded realization.

The planner may therefore choose among front-compatible realizations without requiring their catalog/form names to match.

## Planning

Planning separates **compatibility** from **exact realization**.

Candidate admission begins with front compatibility:

```text
gear's required checked front
        ↓
front-compatible host/form realizations
        ↓
resource + authority + observation + policy filtering
        ↓
selected exact realization
        ↓
immutable plan
```

Once a realization is selected, the plan remains exact. It may seal:

- exact host and boot as appropriate;
- exact implementation and artifact identity;
- resources and reservations;
- authority;
- connections and route candidates;
- finite limits;
- sign requirements.

Functional compatibility therefore does **not** mean runtime improvisation. A compatible realization absent from an already-sealed plan cannot be substituted opportunistically unless the plan explicitly admitted that alternative or a new planning pass produces a new plan.

## Names and revisions

Names and revisions are provenance and catalog facts, not compatibility gates.

Therefore:

```text
same front + different name       -> compatible
same front + different revision   -> compatible
different front + same name       -> incompatible
different front + same revision   -> incompatible
```

A revision change that changes the checked front is naturally incompatible because the front changed. A revision change that leaves the canonical checked front unchanged does not create incompatibility merely by changing the revision token.

Proof and conformance sign remain attached to the exact implementation/artifact/revision that was actually tested. Functional compatibility does not transfer historical proof claims to an untested implementation.

## Identity

Keep these identities separate:

```text
source/form/catalog identity
checked front identity
expanded form identity
selected implementation/artifact identity
plan identity
play identity
sign identity
```

`FaceId` or an equivalent canonical checked-front digest may be useful internally. The exact representation is an implementation choice, but compatibility must derive from the checked front rather than from the source/catalog name.

Two differently named things with the same checked front may have different source/catalog identities while sharing the same compatibility class.

## cords

cord compatibility follows the same functional principle at the connected boundary. Value type, direction, temporal behavior, bounds, and other checked port facts must agree as required by the front contract.

Do not infer compatibility from declaration order, friendly names alone, or implementation technology.

## Catalogs and host families

Catalog categories such as `text/`, `time/`, `flow/`, `web/`, or `llm/` remain useful organization and opt-in packaging boundaries.

A host may advertise named kinds for discovery and signs, but planning eligibility is based on their checked fronts plus other explicit planning requirements. Category prefixes and kind names do not form a nominal type hierarchy.

A host compiled with an opt-in family still advertises only the exact realizations it can currently promise. Functional compatibility does not weaken runtime truth or finite limits.

## Shared pools

A shared pool's member contract is likewise a checked front. A pool may admit members that are functionally compatible with the pool's declared member front even if those members come from differently named forms or host-provided kinds.

Pool identity, member identity, membership epochs, authority, and finite capacity remain exact runtime/plan facts. front compatibility does not make pools ambient or unbounded.

## Diagnostics

Prefer diagnostics such as:

```text
front mismatch
missing startup parameter
runtime port mismatch
temporal shape mismatch
shorthand mismatch
no front-compatible realization
```

over nominal errors such as:

```text
wrong kind name
wrong kind ID
wrong catalog path
wrong revision
```

A name/revision may still appear in a diagnostic to identify the candidate being discussed, but it must not be the reason for incompatibility when the fronts are equal.

## Migration from the nominal checkpoint

PRs #520 and #521 intentionally implemented the then-current nominal rule. That rule is now superseded.

The migration replaced the former expectations that:

- a differently named form with the same front is incompatible;
- an offer with the same front but a different kind identity is ineligible;
- a revision difference alone makes a candidate incompatible;
- structural/front coincidence must be rejected.

The current compatibility contract requires positive and negative proofs:

1. differently named callables with exactly equal checked fronts are compatible;
2. a same-named callable with a changed front is incompatible;
3. planning can choose a differently named front-compatible host offer and still seal its exact implementation/artifact identity;
4. changing only the selected exact realization changes plan identity as appropriate without changing front compatibility;
5. incompatible startup/runtime/temporal/shorthand fronts fail closed.

## Non-goals

This rule does not introduce:

- implicit coercions;
- width/depth structural subtyping;
- variance rules;
- duck-typed runtime dispatch;
- ambient dynamic plugin selection;
- unplanned runtime substitution;
- proof transfer between implementations;
- weakening of resource, authority, transport, or sign exactness.

## Canonical sentence

> **The front is the contract. If the front is the same, it fits. The plan still records exactly what was chosen.**
