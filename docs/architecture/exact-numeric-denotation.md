# Exact numeric denotation and bounded realization

This records the specification, gap audit and development progress for
[#5392](https://github.com/dancxjo/conduit/issues/5392). The bounded rational
foundation below is implemented in Rust. Checked authored numeric Types,
Quantity migration, symbolic evaluation and precision selection remain open.
The [canon](../conduit-canon.md) owns architectural intent; current proof scope
is recorded separately from this proposed implementation direction.

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

## Bounded rational foundation in development

[`ExactRational`](../../architecture/core/src/exact_numeric/rational.rs) retains
an exact signed integer or reduced rational with a positive denominator and one
canonical zero. Numerator, denominator and arithmetic intermediates each have a
4096-bit extent. Decimal parsing is exact and bounded; it performs no float
conversion. The versioned codec uses minimal little-endian magnitudes and
rejects unreduced fractions, negative zero, padding, malformed lengths and
unsupported versions. Semantic digests commit to that canonical reduced value;
author spelling and source custody remain separate evidence.

Addition, subtraction, cross-cancelled multiplication, division and
continued-fraction comparison use fixed storage and an explicit
`ExactNumericWork` budget. Extent or work exhaustion refuses the selected
profile; it does not assert that the mathematical expression is invalid.
Work charges are deterministic algorithmic admission units, independently of
#5395's required wall/CPU measurements. Existing fixed-width hot paths keep their
existing representations; this foundation does not require every operation to
materialize a rational or symbolic graph.

`ExactIntegerProjectionRequest` selects an explicit signed/unsigned fixed-width
integer Type. It refuses fractional coordinates, out-of-range values, negative
unsigned results and unsupported targets. The exact projection domain adapts
the existing general projection report, rechecks the result and target Type,
and rejects omitted obligations or fabricated transformation laws. Floats need
an admitted algorithm/error witness; Count keeps its separate cardinality law.
This API does not implicitly treat either as an integer target.

Current regression proof covers exact thirds and tenths, `10^1000 + 1`, bounded
work and extent refusals, canonical decoding, a small independent arithmetic
oracle and exact fixed-width projection fidelity. These are core Rust proofs,
not checked source/Plan/Play numerical realization acceptance. Next, connect the
rational denotation to ordinary checked transport, canonical Quantity conversion
and offered numerical realization. Reliable bounded arithmetic and type-safe
projection are the scheduling prerequisite; the remaining symbolic features do
not block performance baselines, cost observations, dispatch or placement.

[`RationalQuantity`](../../architecture/core/src/exact_numeric/quantity.rs)
adds an optional bounded rational coordinate representation using the existing
complete Unit capsule. It shares physical conversion eligibility and checked
source correlation with the decimal path: Dimension, nominal family, affine
role and reference anchor must all agree. Rational affine conversion applies
the point offset once; differences receive only the scale. A conversion such
as `1m` into a source-admitted Unit of three meters retains the exact coordinate
`1/3`, while the existing decimal representation still truthfully refuses it.
Receipts keep the checked original Quantity, authored evidence and target Unit
spelling even when work admission refuses the result.

Its separate integer projection retains the precise Unit and role alongside
the machine coordinate. The generic projection report verifies the exact
coordinate, target integer Type and physical contract. A Unit-conversion law
cannot stand in for a numerical-representation law. Tests cover independent
Unit and integer projections, fractional projection refusal, affine points and
deltas, binary/decimal prefixes, incompatible dimensions/families/anchors,
source mismatch and hostile codec data. Integer powers use bounded squaring;
zero power is the multiplicative identity, and a negative power of zero refuses.
The independent Python Fraction corpus also checks all 972 affine conversion
vectors and 798 physical comparison vectors, including cases whose rational
coordinate could not fit the existing finite-decimal profile.

These are core representation and fidelity proofs. They do not yet register an
authored rational Quantity Type, replace the decimal `Quantity` codec, select a
numerical Back or prove ordinary Plan/Play precision admission. Checked transport
and offered realization remain the next integration requirements; symbolic π,
roots and general symbolic reasoning remain separate open work.

The optional outward integer-enclosure projector prepares `floor(value)` and
`ceil(value)` in one selected signed or unsigned machine Type under the caller's
explicit limb-work budget. Both endpoints must fit. It retains the exact source,
actual outward error distances and admitted maximum coordinate error; insufficient
precision, range or work refuses rather than rounding silently. Wide operands
are divided before narrowing, so a large numerator does not by itself prevent a
small quotient from being enclosed.

Preparation seals the source/result/error correlation. The common projection
report inspects that sealed evidence without repeating arithmetic or introducing
an ambient work budget. Fractional source denotation has declared precision loss,
and an exact-only consumer refuses it; a policy admitting the retained error
requirement may consume the enclosure. Integral coordinates keep exact fidelity.
This is an arithmetic containment guarantee, not a confidence interval or a
promise about a future measured execution.

The physical wrapper retains the complete original Unit and role. Its tolerance
must use that exact Unit and a difference role (Delta for an affine Point);
perform any Unit conversion explicitly through the existing physical conversion
law first. For example, an admitted `1ms` coordinate-error requirement encloses
`1/3ms` as `[0ms, 1ms]`, while neither a meter tolerance nor an affine temperature
point is admitted as its error distance. This supplies a reviewed preparation
boundary for future finite scheduling coordinates. It does not yet lower a cost
forecast into dispatch, register a source numeric Type or select an ordinary
Plan numerical Back; those integration requirements remain open.

## Certified principal square-root enclosure in development

[`PreparedSquareRoot`](../../architecture/core/src/exact_numeric/root.rs)
prepares the principal nonnegative real square root of an exact rational
radicand as exact rational endpoints. Exact bisection preserves
`lower² <= radicand <= upper²`; the retained interval width bounds either
endpoint's absolute error. No floating-point constant, library approximation,
or statistical confidence claim participates in the certificate. The source
radicand remains unchanged.

The caller admits a maximum interval width, a finite refinement count and
limb-work budget. Negative radicands and negative tolerances refuse distinctly;
exhausted refinement, arithmetic work or intermediate extent also refuse.
Zero tolerance succeeds only if this algorithm proves an exact root. For
example, `sqrt(4)` produces `[2, 2]`, and `sqrt(2)` with width at most `1/1000`
produces a certified enclosure. A rational root that bisection cannot find
exactly may refuse zero tolerance; this is algorithm eligibility, not a claim
that its mathematical root is irrational.

The existing generic projection report records precision loss for a nonzero
width and exact fidelity for a proved point result. Preparation seals
source/endpoints/width; report inspection performs no arithmetic. Consumer
policy retains the exact admitted width, and altered results, missing
obligations or invented transformation laws fail domain validation.

This is a core Rust preparation and evidence primitive. It does not yet
introduce authored square-root syntax, a symbolic expression codec, physical
Quantity root algebra, or an offered Planner/Plan/Play realization. Those
#5392 integration gates remain open. The fixed-width arithmetic hot path is
unchanged; bisection runs only at an explicitly requested preparation boundary.
