# Callable compatibility and semantic realization

**Status:** canonical architecture direction  
**Applies to:** forms, catalog kinds, host offers, planning, reusable composition, and shared pools
**Related:** #507, #511, #512, #514, #515

## Two relations

Conduit keeps callable fit separate from semantic substitutability.

> **Equal canonical checked fronts mean two things can be called the same way. They do not, by themselves, mean the things do the same work.**

Interface compatibility is exact `CheckedFront` equality. Semantic realization
eligibility additionally requires the candidate to claim the authored gear's
exact semantic contract identity.

Names remain valuable for authorship, discovery, catalog organization, provenance, diagnostics, sign, and exact realization records. They are not hidden nominal types.

Ordinary realization therefore uses both gates:

```text
same canonical checked front + same semantic contract -> eligible
same front + different semantic contract               -> ineligible
different Front                                         -> ineligible
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

The Back does not participate in interface compatibility or semantic contract
identity. Two materially different implementations may realize the same
semantic contract, while two identical-looking `Text -> Text` operations may
mean uppercase and redact and must not substitute for each other.

Terminal, liveness, effect, and domain laws that affect substitutability belong
to the semantic contract identity even when they do not alter the callable
Front. Resource, authority, and Host Call requirements remain later exact
admission gates; they do not define the operation's meaning.

## forms and kinds share the same compatibility law

A reusable form and a host-offered primitive kind are not separate compatibility universes.

Conceptually:

```conduit
form loud (
    text: Text >> text: Text
) {
    upper: text/upper
    text >> upper >> text
}
```

If another callable thing has the same checked front as `loud`, it fits that
boundary. It realizes `loud` only when it also declares the same semantic
contract. It may still be:

- another reusable form;
- a standard catalog kind;
- a host-native implementation exposed through a kind offer;
- a browser/WASM realization;
- a bounded embedded realization.

The planner may choose among Front-compatible realizations with the same
semantic contract without requiring their catalog/form names or Back identities
to match.

## Planning

Planning separates **compatibility** from **exact realization**.

Candidate admission begins with both compatibility relations:

```text
gear's required checked front + semantic contract
        ↓
Front-compatible realizations of that semantic contract
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

Friendly names are provenance and catalog facts, not compatibility gates. The
current `KindIdentity` value is the immutable semantic-kind contract
identity: despite its historical name, it is not merely a display version and
must eventually be derived from the reviewed semantic contract under #3712.

Therefore:

```text
same front + same contract + different name/back -> eligible
same front + different contract                  -> ineligible
different Front + same contract                  -> ineligible
```

A semantic-contract change remains incompatible even when the front does not
change. Implementations and artifacts remain exact selected realization facts;
they are deliberately absent from semantic identity.

Proof and conformance sign remain attached to the exact implementation/artifact/revision that was actually tested. Functional compatibility does not transfer historical proof claims to an untested implementation.

## Identity

Keep these identities separate:

```text
source/form/catalog identity
checked front identity
semantic contract identity
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

A shared pool declaration is the canonical structural higher-order case in the
current language: it explicitly declares a member Front and bounded membership,
so its authored meaning is to accept any exact front-compatible member. This is
not the default rule for ordinary gears. A future pool syntax that promises one
particular worker behavior must additionally carry that semantic contract.

Pool identity, member identity, membership epochs, authority, and finite capacity remain exact runtime/plan facts. front compatibility does not make pools ambient or unbounded.

## Diagnostics

Prefer diagnostics such as:

```text
Front mismatch
semantic contract mismatch
missing startup parameter
runtime port mismatch
temporal shape mismatch
shorthand mismatch
no semantically eligible Front-compatible realization
```

over nominal errors such as:

```text
wrong kind name
wrong kind ID
wrong catalog path
wrong revision
```

A name/revision may still appear in a diagnostic to identify the candidate being discussed, but it must not be the reason for incompatibility when the fronts are equal.

## Explicit structural polymorphism

An authored operation whose meaning really is “any callable with this exact
Front” uses the reviewed `conduit.semantic/structural-polymorphic@1` contract.
Only the requirement side may use this marker. It is not an ambient planner
fallback and does not let an offer grant itself broader eligibility.

The migration replaced the former expectations that:

- a differently named form with the same front is incompatible;
- an offer with the same front but a different kind identity is ineligible;
- a revision difference alone makes a candidate incompatible;
- structural/front coincidence must be rejected.

The compatibility contract requires positive and negative proofs:

1. equal-Front callables with different semantic contracts do not substitute;
2. differently named/implemented callables with one semantic contract remain eligible;
3. an explicit structural-polymorphic requirement accepts any equal Front;
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

> **The front says how to call it. The semantic contract says what it means. The plan records exactly what was chosen.**
