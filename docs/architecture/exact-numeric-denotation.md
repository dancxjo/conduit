# Exact numeric denotation and bounded realization

This is the specification and gap-audit slice of [#5392](https://github.com/dancxjo/conduit/issues/5392).
It proposes the next implementation contracts. It does not implement a new
numeric Type, symbolic evaluator, precision selector, or public syntax. The
[canon](../conduit-canon.md) remains the description of current implementation.

## Meaning and realization

An ordinary mathematical value denotes its exact mathematics when the system
supports a finite expression for that meaning. A consumer imposes numeric
requirements; the Planner selects an offered, finitely admitted representation
and algorithm satisfying those requirements. Approximation belongs to that
projection, with inspectable evidence. It must not overwrite the source value.

Universal here describes independence from a particular machine representation.
It does not assert that every real number has a finite encoding, or that every
symbolic equality is decidable. Every transported value, expression graph and
execution still needs explicit finite admission bounds.

## Implementation audit

The physical-value rows describe #5391's implementation. They do not claim that
#5390's independent Dimension transport acceptance is complete.

| Existing surface | Current behavior | Remaining #5392 work |
| --- | --- | --- |
| [Scalar](../../architecture/core/src/info.rs) | Signed `i64` micro-units with scale 1,000,000. Multiplication and division use integer intermediates and truncate to that scale. | Keep this as an explicit finite realization; do not identify its rounded result with exact rational mathematics. |
| [Quantity codec](../../architecture/core/src/quantity/exact.rs) | A 38-digit decimal coefficient, bounded decimal exponent, Unit capsule and point/difference role. Nonterminating decimal conversion refuses. | Retain an exact rational result when a selected decimal coordinate cannot represent it. Migrate the canonical value contract deliberately rather than add another competing public quantity carrier. |
| [Conversion law](../../architecture/core/src/quantity/conversion_law.rs) and [wide arithmetic](../../architecture/core/src/quantity/magnitude.rs) | Rational intermediates implement exact unit laws within fixed storage; the magnitude workspace holds 576 decimal digits. | Reuse the laws and explicit bounds; this private workspace is not a universal numeric value or a Planner precision witness. |
| [Physical declarations](../physical-declarations.md) | Dimensions, families, roles and rational/affine Unit relationships come from immutable source definitions. Turn and degrees relate exactly; radian has an independent anchor. | A checked symbolic law may express `1turn = 2π rad`. Preserve definition custody and family/role checks; never insert an approximate rational constant for π. |
| [Expression programs](../../architecture/plot/src/expression_program.rs) and [evaluation](../../architecture/plot/src/expression_evaluate.rs) | Programs preserve checked value Types and admitted constants, then evaluate according to those concrete numeric Types. | Preserve exact mathematical expression meaning independently of a consumer's representation. Explicit IEEE computation must keep its own operation order and rounding semantics. |
| [Realization requirements](../../architecture/planner/src/requirements.rs) | Hard predicates, resource ceilings and current offers constrain selection. | Add checked numeric requirements and an algorithm-specific satisfaction witness; a characteristic label claiming precision is insufficient. |
| [Generic projection model](../../architecture/core/src/projection/model.rs) | Precision, range and approximation already have common loss categories and domain-owned detail. | Supply numeric detail and verified evidence through this contract, following #4952 and #4051; do not create a parallel numerical loss-reporting system. |

The fixed-point and decimal representations remain useful bounded paths. Their
current limits must be reported honestly until the canonical semantic-value
migration is implemented and tested.

## First executable subset

The next slice supplies exact integers, normalized rationals and exact decimal
parsing. A rational has a positive denominator; numerator and denominator are
coprime; zero has one canonical encoding. Decimal spelling denotes a rational
power of ten, with source spelling and spans retained separately. Division by
zero is a domain refusal. Integer division, if offered, has its own explicit law.

Finite storage and work bounds are admission facts, not changes to the
mathematical domain. A compact expression such as `10^1000 + 1` may remain an
exact denotation under admitted node and exponent bounds. Materializing its
integer digits requires a separately admitted extent. An `i64` Back refuses
that realization without replacing the value by overflow, saturation or a float.

Cheap bounded integers may use an inline representation. They must not require
symbolic graph allocation or arbitrary-precision evaluation on their hot path.
Any larger exact representation is charged by its actual admitted storage and
worst-case operation work before Play.
Count retains its nonnegative cardinality/index contract; sharing exact numeric
machinery does not erase that domain or permit fractional counts.

An extensible expression representation uses a versioned, bounded acyclic graph
with explicit node, edge, depth, integer-extent and rewrite-work limits. Decoder
admission rejects cycles, invalid references, noncanonical literals and forged
definition identities. Canonical structural identity supports hashing and
transport correlation; mathematical equality is a separate operation.

## Symbolic constants and comparison

π denotes a pinned named constant, not its decimal expansion. `π/180` and
supported roots retain their exact denotations through ordinary checked value
transport. Only admitted rules may simplify them. A later finite projection
returns a certified enclosure or a justified numeric result with its error
bound, or a precise refusal. General symbolic comparison may be unproved;
structural inequality alone does not establish mathematical inequality.
Comparison outcomes distinguish proved equal, proved different, unproved and
refused; an unproved or incompatible result must not become Boolean false.
Root nodes also retain the checked root definition and branch/domain, so
transport cannot silently select another root or reinterpret a later rebinding.

The rational subset proves `1/3 + 1/3 + 1/3 = 1` and
`0.1 + 0.2 = 0.3` exactly. These identities do not authorize reassociation of
explicit IEEE operations, nor do they claim a general computer algebra solver.

Angle remains a quantity family with turn as its exact reference coordinate.
Ordinary trigonometry consumes Angle, so bare `sin(42)` and `sin(1m)` refuse
checking. Reviewed exact special cases can prove `sin(0.25turn) = 1`.
Undefined cases such as `tan(0.25turn)` need a typed domain refusal. Public Kind
spellings and expression syntax require their own reviewed implementation slice.

## Consumer requirements and Planner evidence

Each consumer declares exactness or an admitted numerical error policy, input
range/domain assumptions, rounding and overflow behavior, reproducibility, and
finite memory/work/latency constraints. The requirements are associated with
the checked semantic expression and the consumer boundary, not mutable ambient
numeric settings.

An eligible Back offers both a representation and an algorithm. Its checked
witness identifies the input assumptions, intermediate rounding and operation
order, final error/range guarantee, and finite resource bounds. Merely rounding
the final exact answer does not prove the error of a floating-point algorithm.
Unknown or unsatisfied assumptions refuse eligibility; a trusted contract must
be identified explicitly rather than presented as a derived proof.

One expression may feed an exact rational consumer and a consumer selecting an
admitted `f64` approximation. Their Plans retain the same source meaning and
record distinct realization and projection evidence. No suitable offer means
an admission refusal before Play where determinable. Replanning must recheck
the requirements and cannot silently relax precision. Execution-time breaches
retain explicit safe failure outcomes.

Certified numerical approximation is distinct from measurement uncertainty.
Explicit IEEE-value semantics also preserve signed zero, NaN, infinities,
bit identity, and the specified rounding/exception behavior. They must not be
silently folded into ordinary exact mathematical equality.

## Delivery and proof gates

1. Implement the rational spine and its bounded canonical codec, exact decimal
   parsing, arithmetic and equality. Prove exact identities, normalization,
   corrupt-payload rejection, bound refusal, and a cheap integer path on std
   and no_std targets.
2. Migrate Quantity numeric coordinates using the existing source-defined Unit
   laws. Prove a nonterminating decimal conversion retains a rational result,
   and preserve affine points, differences, nominal families and source custody.
   Independent Dimension transport remains owned by #5390.
3. Admit numerical requirements and witnesses through ordinary offers, Planner
   selection, Plans and generic projection evidence. Prove two consumers of
   one meaning, unsatisfiable constraints, intermediate cancellation/rounding,
   and refusal when an error bound cannot be established.
4. Add bounded named constants/roots and certified projections, including the
   π-bearing turn/radian law. Prove exact symbolic transport and explicit
   unproved comparison; do not block the rational spine on a general CAS.
5. Exercise one DSP and one tensor numeric consumer through the same contracts.
   Record browser, std, no_std, hosted CI and physical evidence separately.

All gates above remain open. This document starts #5392 in a separate reviewable
slice; it neither expands #5391 nor closes #5392.
