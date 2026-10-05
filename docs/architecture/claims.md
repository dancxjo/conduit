# Claims and deterministic resolution

Signs establish bounded evidence of what was observed or happened. Claims
interpret evidence as typed assertions. Resolutions select or abstain under an
exact domain policy. Authority independently decides which effects are allowed.
None of these contracts substitutes for another (#4950).

## The envelope

[`conduit_core::claims`](../../architecture/core/src/claims.rs) owns admission,
immutable assertion identity, explicit lifecycle history, score comparability,
and resolution receipts. `ClaimDomain` owns the exact target and typed value
and validates their entire finite representation. Its contract identity names
that reviewed target/value contract. There is no universal entity taxonomy,
untyped JSON payload, source-priority table, or mandatory per-body claim store.

Preparation borrows frozen domain values, provenance, exact Sign identities,
source generations, and claim references. IDs and rationale text admit at most
192 UTF-8 bytes each. A claim admits one to sixteen support references and at
most sixteen conflicts; duplicates, self-edges, and simultaneous support and
conflict refuse. A producer and exact artifact identify provenance. Domains
may use a source-generation reference for human annotations or external data.
References do not prove evidence exists or give authority to access it; the
producer/admission boundary must establish their authenticity and availability.

The envelope exposes read-only target, value, provenance, score, and rationale.
Rank and score cannot replace identity. Updating an assertion requires explicit
later truth; changing its lifecycle never rewrites its assertion. Distinct
producers must preserve existing IDs and create fresh IDs for distinct claims.
Global identity issuance and durable storage remain domain obligations.

`ClaimScore` retains an exact scale, optional calibration, producer, inclusive
integer bounds, and score. It has no numeric ordering implementation. Explicit
comparison refuses unless all five contract facts agree. Domain policy may
establish a reviewed conversion outside this generic mechanism; equal numeric
ranges alone never establish comparability.

## History and resolution

A proposed claim can stabilize, revise, or invalidate. A stable claim can
commit, revise, or invalidate. Revised and invalidated claims remain inspectable
but cannot win. Revised truth names a replacement identity; invalidation names
its reason. Commitment is terminal for that assertion. Correcting committed
truth creates a new claim explicitly supported by the old one or exact later
correction evidence. A claim may lose selection while remaining stable or
committed; policy owns selection, not lifecycle. Four fixed history slots bound
all legal transitions; each transition records its state, reason, and optional
replacement, in sequence order.

`resolve_claims` admits one to thirty-two candidates for one exact target and
claim contract, in ascending durable-ID order. The extended
`resolve_claims_with_evidence` accepts a separate closure of at most thirty-two
claims about other targets in the same domain. Each candidate must be the same
immutable object as its closure entry, not another object with the same ID.
Every claim-support reference resolves in that closure. A bounded graph check
rejects support cycles; symmetric conflict edges are permitted. External or
cross-domain observations can remain exact source-generation support.

A `ClaimPolicy` is a reviewed pure function. Its identity must bind its version
and configuration; its only decision inputs are the immutable candidate set,
eligibility, and supplied evidence. It must not consult clocks, randomness,
mutable provider state, or an unrecorded source-priority list. Core admits the
receipt and fences illegal decisions; it does not sandbox arbitrary policy
code or prove that an arbitrary implementation is pure. Domain conformance
must prove repeatability for that exact policy.

Every receipt retains all candidate identities, lifecycle revisions, explicit
exclusion reasons, exact policy identity, and the selected identity or a
first-class abstention with machine/human rationale. It borrows the exact
claims and evidence, making support, conflicts, losing values, history, and
provenance inspectable. The immutable borrow prevents those objects changing
under a retained resolution. Evidence explaining a runtime resolution can be
recorded separately as ordinary Signs; resolution does not fabricate Signs.

A conservative consumer may intersect facts across its bounded plausible set.
The domain owns both plausibility and intersection: sharing token endpoints
does not imply sharing a dependency relation. No generic winner or intersection
algorithm is imposed. Explanations concern supplied evidence and policy, never
private model reasoning.

All generic operations use borrowed data and fixed arrays and allocate nothing.
Domain validators and policies must also honor their admitted bounds. This is
a preparation/semantic library, not a new scheduler, kernel runtime operation,
Host offer, authored kind, or automatic Patchbay visual layer. Focused inspection
can consume the read-only receipts without owning truth. Storage persistence,
Sign publication, policy execution admission, and focused Patchbay wiring remain
consumer responsibilities. Nothing in a claim or resolution issues a capability,
changes a plan, or requests a Host Call.

## Deterministic development proof

Run `cargo xtask check workspace-test-foundation` for the core and language
proofs with the rest of the foundation contracts.

- Core sensor fusion retains direct observation, rule, learned classification,
  and manual correction. A fixture-specific manual policy selects the correction
  repeatably while preserving the losing model value, score contract, and edges.
- Language consumes the existing native revisioned token and dependency arc
  types. Vocative and apposition proposals retain exact parser/model provenance,
  conflicts, stable/revised history, and conservative abstention. A domain
  consumer extracts shared endpoints without claiming a shared relation. This
  establishes the #4907 consumption seam, not the complete streaming parser.
- Runtime diagnosis retains one exact timeout Sign and both host-unavailable and
  path-unavailable claims. Insufficient evidence causes explicit abstention.
- Negative conformance covers text/edge/candidate bounds, duplicate identities,
  canonical order, cycles, unresolved support, target mismatch, substituted
  evidence, score incompatibility, commitment locks, and illegal selections.

These are deterministic library fixtures. They do not establish physical sensor
accuracy, learned-model quality, a live parser, production diagnosis, or accepted
release evidence.

The small envelope/history/retained-alternatives idea comes from
[Tongues' pinned linguistic-claims contract](https://github.com/dancxjo/tongues/blob/b03702798db5a5e7d174278ac2fc1233f171b02e/docs/linguistic-claims.md).
Its universal priority ladder and normalized confidence ranking are deliberately
not reused; provenance is recorded in the [reuse ledger](../reuse-ledger.md).
