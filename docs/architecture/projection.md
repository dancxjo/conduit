# Bounded semantic projection fidelity

[Issue #4952](https://github.com/dancxjo/conduit/issues/4952) owns the development
library seam in `conduit_core::projection`. A projection may lose meaning; an
exact policy may permit that loss; loss must remain visible.

Representation (#4428) specifies an authoritative encoding of a value under a
compatibility contract. Projection maps obligations between contracts. Its
report does not replace either contract, the target artifact, Plan selection,
or Sign evidence of execution. Ordinary Backs need a report only when a real
semantic projection boundary exists.

## Admission and meaning

`ProjectionDomain` owns the finite source, target and typed loss detail. Its
reviewed validator must check the **complete source obligation inventory**,
actual target semantics, typed losses, native evidence and scores. It receives
the mechanism outcome so an unattempted refusal need not pretend to have a
projected artifact. It must reject silent omissions, unsupported preservation,
unknowns coerced into portable meanings, malformed domain data and unbounded
storage. These pure validators are semantic contracts, not hostile-code
confinement. Core cannot discover domain meaning from arbitrary bytes.

Every obligation has one bounded identity and exactly one `Preserved`,
`Transformed { law }` or `Lost { class, detail, native_fact }` fact. A target
appropriate transformation can be exact: a semantic group becomes a named
linear record rather than spatial layout. The domain checks the exact law and
target contract. Lost facts classify unrepresentability, unrecognized target
meaning, precision, range, cardinality, ordering, temporal/finality, identity,
provenance, unsupported variants, truncation or approximation. Domains supply
their own detail; core supplies no universal adapter ontology.

Admission computes `Completed(Exact)` only when a target exists, all required
obligations are accounted for and no loss facts exist. With losses,
`Completed(PermittedLossy)` additionally requires the exact
`ProjectionPolicy::permits` decision over the **whole inventory**, source and
target. This supports aggregate budgets rather than unlimited per-fact
permission. Otherwise the result is `Insufficient`, including a mechanically
completed boundary with no useful target. There is no universal best-effort
policy. `Refused`, `Failed` and `Cancelled` retain canonical `TerminalInfo`
category and causal digest, distinct from semantic insufficiency. A failed or
unattempted mechanism cannot report a successfully preserved target.

`require_exact` refuses partial and permitted-lossy targets. `require_policy`
requires the exact admitting policy identity and a completed result. Inspection
may separately borrow a partial target without implying semantic acceptance.
The immutable report preserves its source, target, projector, requested route,
source/target contracts, policy and optional exact expanded-form boundary.
Caller-supplied identities correlate artifacts; they are not cryptographic
attestation or ambient permission. Domains and execution evidence own the
truth of those correlations.

## Finite native truth and scores

Preparation freezes borrowed data. Core admits at most 64 obligation facts,
16 native facts of at most 4,096 bytes each, eight scores, eight admitted
realizations/attempts and eight diagnostics. Identifiers and diagnostic wording
are nonempty and at most 192 UTF-8 bytes. Overflow is typed admission refusal;
it does not silently truncate the inventory. Actual truncation must be a typed
loss under an exact admitting policy. Domain validators must bound their own
source/target/detail storage and mandatory work. Core stores no growing queue,
and its operations allocate no storage; domain preparation remains responsible
for its declared finite work and any hosted preparation allocations.

Unknown native facts retain exact identity, external contract, provider,
encoding and bounded opaque bytes. An unrecognized loss must reference retained
native evidence. Retention does not promote a provider label into semantic
identity. Unknown `XYZ` need not become `dep`, `other` or candidate zero.

`ProjectionScore` reuses the existing `ClaimScore` scale, calibration, producer,
bounds and integer-profile law. Native scores correlate exact native facts and
must retain the same provider. Comparison refuses differing score contracts;
there is no blanket numeric ordering. `Normalized` retains the original and
projected scores and an exact normalization policy. Normalization refuses by
default; an explicit reviewed policy must validate the numerical mapping.
Domains additionally validate native score semantics. This is not confidence
calibration by a cast. Unsupported native numeric formats may remain native
evidence without coercion into the integer score profile.

## Attempts and inspection

Attempts reference exact already-admitted Plan/Host/boot/Back/artifact tuples.
Each retains its terminal outcome. Later alternatives require exact fallback
reasons; selection names the final completed attempt. Duplicate retries,
unadmitted realizations and further trials after completion refuse. These are
correlations with supplied admission facts, not a substitute for validating a
Plan or proof that a mechanism ran. Pure preparation reports may have no
execution attempts. This seam performs no retry, discovery or replanning.

`PatchbayGraph::inspect_projection` resolves an exact selected subject against
the graph and report's expanded-form boundary. It returns a concise computed
summary and the **same borrowed report** for typed fact, native evidence,
score and attempt drill-down. Stale references, another boundary and missing
boundary facts refuse. Patchbay owns no second projection store; it does not
add badges to every Cord or manufacture a report where no boundary exists.

## Proof and scope

The supported repository proof entrance is:

```sh
cargo xtask check workspace-test-foundation
```

Focused deterministic fixtures cover:

- supplied native language arcs: one recognized relation, retained unknown
  relation, token-alignment mismatch and exact native parser cost; partial
  strict refusal, silent omission and coercion negatives;
- a native portable speech phone whose aspiration distinction a checkpoint
  symbol cannot express; explicit one-loss policy and strict refusal, without
  checkpoint symbols becoming phone identity;
- actual linear rendering of a Face with labels, accessible text, an action,
  ordering and grouping; exact target transformation and omitted-action
  negatives. Authored Face contains no geometry to lose;
- actual checked/expanded Form projection into Patchbay, exact boundary
  resolution and borrowed report drill-down;
- generic loss classes, aggregate budgets, finite admission, terminal outcomes,
  score contracts and exact admitted fallback identities.

These establish development-library conformance, not live parser accuracy,
model acoustic quality, physical/human presentation proof, new report-producing
Kinds, every adapter's integration or stable-release acceptance.

The reviewed precedent is pinned Tongues
[`grammar-parser.md`](https://github.com/dancxjo/tongues/blob/b03702798db5a5e7d174278ac2fc1233f171b02e/docs/grammar-parser.md):
native output separated from portable projection, unknown/alignment facts,
requested/attempted/selected backends, explicit fallback and native costs.
Reuse is limited to those laws. Backend discovery, subprocess execution,
unbounded diagnostics and native confidence coercion are not imported.
