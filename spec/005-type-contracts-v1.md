# Domain-owned type contracts and registry version 1

Status: stable

Type-contract reference version: 1

## Purpose

Conduit routes values by exact semantic type without owning domain ontologies.
A portable plan can retain an opaque type reference; a hosted registry can
discover the exact immutable descriptor and ask its owning provider the
directional question “may this consumer accept every value from this
producer?”

The core does not infer type meaning from a name, serialized shape, language
type, byte layout, transport, or implementation.

## Portable reference

`TypeContractRef` contains:

- a portable namespaced contract identifier;
- the exact domain-owned contract schema revision; and
- the canonical semantic hash of the exact contract descriptor.

The identifier contains exactly one `/`. The prefix selects a hosted provider;
neither prefix nor suffix is enumerated by `conduit-core`. The schema revision
does not imply compatibility. A reference is immutable and does not embed a
provider, implementation, artifact, host observation, or presentation label.

## Hosted provider registry

Providers are registered by one portable local namespace. Registration order
does not affect lookup or decisions, and duplicate namespaces are rejected.
A provider can:

1. discover an exact canonical descriptor and a non-semantic human name; and
2. answer `consumer_accepts_producer(consumer, producer)` with compatible,
   incompatible, or indeterminate plus a stable provider-owned rule.

The registry verifies that a discovered descriptor's kind, schema revision,
and semantic hash match the requested reference. A provider cannot replace or
mutate a descriptor after its identity has been named. Dynamic loading, FFI,
process protocols, and provider distribution are outside this contract.

Specification
[`027-implicit-satisfaction-v1.md`](027-implicit-satisfaction-v1.md) extends
the hosted provider contract with an exact immutable provider descriptor and a
`TypeSatisfactionReport`. The report retains the existing decision plus both
provider identities, the stable provider rule, and bilateral structural facet
hashes so a resolver can construct a plan-recorded proof. It does not change
the frozen comparison strategies or make structural comparison a fallback.

Missing providers and unknown exact contracts are indeterminate, not
incompatible. Their stable reasons name the unavailable namespace or contract.

## Comparison strategies

The exact domain descriptor declares one strategy:

- **nominal**: exact reference identity is sufficient; any non-exact
  directional acceptance requires a provider rule;
- **structural**: both descriptors explicitly opt into comparison and name the
  canonical semantic hash of the domain-defined structural projection; or
- **opaque**: only the consumer's domain provider can decide.

Exact identity is compatible without loading a provider. Equal structural
projection hashes are compatible only when both sides selected structural
comparison. Different projection hashes are incompatible. A structural and
non-structural pair is incompatible rather than falling back to shape
guessing. Unknown strategies are indeterminate.

A structural projection includes every fact its domain considers meaningful,
including units, scale, bounds, coordinate frame, clock, sensitivity, and
terminal meaning. Two wire records with the same fields but milliseconds
versus samples therefore have different structural identities unless the
domain explicitly defines otherwise.

For nominal and opaque non-exact comparisons, the provider selected by the
consumer namespace returns a stable rule. An adapter rule describes an
explicit node; its existence does not make the unadapted endpoints compatible.

## Decision and diagnostic behavior

The compatibility query retains both exact operands in consumer/producer order.
Stable registry reasons are:

| Reason | Outcome |
|---|---|
| `type-contract-exact` | compatible, exact |
| `type-structural-accepted` | compatible, accepted |
| `type-structural-mismatch` | incompatible |
| `type-strategy-mismatch` | incompatible |
| `type-strategy-unknown` | indeterminate |
| `invalid-type-reference` | indeterminate |
| `type-provider-unavailable` | indeterminate |
| `type-contract-unknown` | indeterminate |
| `type-descriptor-invalid` | indeterminate |
| `type-provider-accepted` | compatible, accepted |
| `type-provider-rejected` | incompatible |
| `type-provider-indeterminate` | indeterminate |
| `type-provider-decision-invalid` | indeterminate |

Provider decisions carry the provider-owned rule as the local subject. No
reason-free Boolean compatibility API is provided.

## Fixtures and compatibility notes

`conformance/c2/type-contract-v1.tsv` is normative. It covers exact nominal
success, provider-declared directional success, explicit structural success,
same-wire-shape/different-semantics failure, an opaque pair requiring an
adapter, an unavailable provider, an unknown strategy, and a malformed
reference. Every row asserts the exact ordered operands, outcome, class,
stable reason, and explanation subject.

Adding a new comparison strategy or changing one of the stable reasons is a
semantic compatibility change. Providers may add new contracts and rules, but
changing the answer for an existing pair changes provider semantics and must
produce new exact descriptor identities where descriptor meaning changed.

## Normative requirements

| ID | Obligation |
|---|---|
| TYP-001 | Keep core references opaque, namespaced, versioned, and exact |
| TYP-002 | Keep registry and provider behavior outside allocator-free core |
| TYP-003 | Ask compatibility in consumer/producer direction with both operands |
| TYP-004 | Return a three-outcome decision, stable reason, and provider rule |
| TYP-005 | Treat missing providers and unknown contracts as indeterminate |
| TYP-006 | Verify discovered descriptors against the referenced identity |
| TYP-007 | Require bilateral explicit opt-in for structural comparison |
| TYP-008 | Never use structural similarity as an implicit fallback |
| TYP-009 | Preserve explicit adapters as nodes rather than compatibility |
| TYP-010 | Reject malformed references and provider decisions deterministically |
| TYP-011 | Keep domain types and meanings out of `conduit-core` |
| TYP-012 | Keep provider loading and language bindings outside version 1 |
