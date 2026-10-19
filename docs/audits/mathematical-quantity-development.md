# Mathematical Quantity coordinates (private Core prototype)

The existing exact Quantity port profile admits decimal v1 and rational v2
coordinates. The Source symbolic pipeline instead carries dimensionless
`MathematicalValue`; it cannot by itself prove preservation of physical meaning.
This private continuation under #5392 introduces a proposed optional v3 Quantity
coordinate form containing the existing checked Unit and role plus canonical
mathematical data. It creates no new Unit catalog, physical Type, literal syntax,
binding authority or numerical evaluator.

`MathematicalQuantityView` borrows its bytes and delegates the coordinate codec
to the existing 32-node/depth-16 mathematical-value validator. Its finite extent
is `2 + UNIT_ENCODED_LEN + MATHEMATICAL_VALUE_MAX_ENCODED_BYTES`. Unit definition
validation and role admission reuse the existing physical law. The declared
family/role/dimension guard is shared with v1/v2 without changing that contract.
Representation identity includes the complete Unit, role and coordinate bytes;
it is not physical equality across different coordinate conventions. A known
rational coordinate can be explicitly materialized as the existing
`RationalQuantity`. An unresolved symbolic coordinate refuses that operation;
it is never silently rounded or stripped of its Unit.

The encoder accepts an already validated mathematical view, checks role, extent,
output capacity and metered copy work before mutating output. Source spelling,
conversion receipts and definition/binding authority remain independent custody;
this form alone does not attest any of them. Neither the legacy
`QuantityValueView`, default primitive validator, nor decimal/exact port profile
accepts v3. An explicit optional `Mathematical` port profile now negotiates v1/v2/v3;
Decimal and Exact retain v1 and v1/v2 respectively. The existing canonical wire
property uses code 3 for the new grant and keeps omitted ports Decimal. Ordinary
`QuantityValueView::decode` still rejects v3; selected profile decoding alone
returns a borrowed mathematical-coordinate view. Unresolved coordinates refuse
rational or decimal materialization. An explicit private Std offer and installed Back now admit the bounded Angle
subset described below, including ordinary Source offer preparation. An unfamiliar form remains a named refusal.

## Unit coordinates and numerical realization remain separate

The Core conformance case checks `0.25turn = 90°` with the existing rational
physical conversion law. Each input then uses the existing checked
`PreparedAngleReferenceConversion` and `physical/angle-turn-radian-2pi@1` law to
produce an exact Pi-bearing radian coordinate. Original spellings and original
Unit identities stay in those receipts. The two equal physical inputs produce
the same proposed radian-coordinate form without inventing rational Pi.

`PreparedMathematicalQuantityEnclosure` then uses the existing borrowed interval
engine to realize those coordinates. It retains the complete physical source
and returns rational endpoint Quantities in the same Unit and role. Numerical
realization has no target-Unit argument and cannot silently select another
coordinate convention. The width is explicitly a coordinate coefficient,
not an affine physical point. A separate point/difference test retains
Fahrenheit coordinates during numerical realization and applies the existing
affine offset only during a later point-to-Kelvin coordinate conversion.

All scratch is prepared before evaluation. Container/vector payload has a
checked pre-allocation lower bound and an actual-capacity check; allocator
bookkeeping, stack, caller-owned input and returned endpoints are excluded.
Frame decoding and coordinate evaluation share a ceiling that a larger caller
grant cannot loosen. Numeric limb-work accounting is not a CPU-time or WCET
claim; Unit decoding is an independently fixed-size operation. Refusals charge
the work already spent and return no substituted value.

## Prepared exact Unit conversion

`PreparedMathematicalQuantityConversion` now changes the Unit frame of supported
symbolic coordinates without numerical realization. It derives affine scale and
offset by converting zero and one through the existing role-aware
`RationalQuantity` law, then imports the coordinate into the existing bounded
mathematical builder and forms `scale * coordinate + offset`. The returned
computational receipt retains the complete source frame, target Unit, exact
scale/offset, output length and emitted-frame digest. The digest is computed
under reserved work after atomic encoding; it identifies that output rather than
asserting cross-Unit byte equality. Source spelling, Unit binding and Plan
authority remain separate custody.

Point and difference roles remain unchanged; the existing physical law supplies
an offset only for points. Distinct incompatible origins still refuse, including
a general turn-to-radian conversion: that bridge continues to require its
separate exact Pi law. Numerical precision is not a conversion argument.
Prepared builder and coordinate buffer payload are bounded before execution;
caller work is clamped to the prepared ceiling and failures charge spent work.
Output is mutated only after all transformation, encoding and copy preflights
succeed. Private scratch resets for the next transaction.

Tests independently build the expected Pi expressions, verify Fahrenheit scale
and offset, then enclose converted Pi-centimetres and check physical endpoints
against independent Pi brackets. Guarded repeated conversion, enclosure,
refusal and recovery paths observe zero allocations and stable prepared payload.

The conversion receipt also adapts to the existing general projection report:
physical denotation and family/role are preserved, while Unit coordinates are
explicitly transformed by the reused exact affine law. The domain binds the
actual emitted frame, not any arbitrary same-Type target. The policy pins the
requested target Unit. Tests construct this exact Unit report followed by the
separate lossy numerical report, with continuous frame identity between them.
Changed target values, Units, roles, policies or obligation inventories refuse.
These are computational reports; they do not attest Source/Plan/Sign custody.

## Physical projection evidence

The coordinate enclosure now adapts to the existing generic `ProjectionReport`
through the same physical-obligation pattern as Quantity integer enclosures.
It delegates numerical obligations and width loss to the existing borrowed
interval domain, adding preservation of the complete Unit and role. The target
contains coordinate bounds in that retained frame. A different Unit, point vs
difference role, or bound fails validation; numerical tolerance cannot authorize
those substitutions. Omitted physical obligations and false exactness also
refuse. Exact rational point intervals may be exact, while nonzero symbolic
width remains reported as precision loss under the selected policy.

The numerical policy fixes coordinate-width requirements in that Unit. It is
not a physical point tolerance, general Unit conversion, Source provenance
attestation or installed projection custody. Source bytes remain available on
the checked enclosure; binding them to Source/Plan/Sign remains separate work.

## Evidence and remaining gates

Guarded tests cover the two angle conversions followed by coordinate enclosure,
physical family/role preservation, strict legacy rejection, malformed/oversized
frames, invalid roles, short-output and exhausted-copy-work atomicity, prepared
storage/work limits, and recovery. Point and difference roles remain distinct,
including under later affine conversion. Tests compare against independent
radian brackets and retain zero observed execution allocations and stable
prepared capacity. Existing v1/v2 role, rational transport and wire-contract
cases remain in the selected regression suite. The checked-contract entrance is
also exercised with an authored physical range: v1 retains its in/out-of-range
outcomes, while both in-range rational v3 and symbolic v3 refuse under every
existing profile. Codec validity alone cannot bypass a range or grant a profile.

The first compile exposed a variable-shadowing mistake during extraction of the
shared role guard. It was corrected before the regression suite, which explicitly
checks wrong-role rejection in both legacy and proposed forms. That diagnostic
is retained with the local capture.

This is private Core conversion/computation proof. Required follow-up remains:
reviewing the form and its explicit wire-profile negotiation, checked Source/Plan and
broader installed Quantity expressions and refinements, Source/Plan/Sign custody
for the general projection report, fresh candidate
and combined integration/publication. Qualified whole-float algorithms and all
original parent requirements remain authoritative. No checkbox is closed here.

The selected mathematical profile validates the same physical Kind before
applying authored checked-contract predicates. A representation grant does not
implement symbolic ordered comparison. Physical range predicates now reuse the
existing exact rational Quantity comparison for rational-shaped v3 coordinates,
including cross-Unit coordinates and inclusive/exclusive endpoints. Unresolved
symbolic coordinates still refuse a range requiring order; no interval or IEEE
projection is silently selected to satisfy it. Tests compare v3 outcomes against
the existing v2 physical comparison and cover refusal followed by recovery.
The extended view retains borrowed bytes rather than an owning symbolic DAG;
its fixed size stays within the existing Unit plus 64-byte envelope.

## Planner and lowered Fore fences

The optional producer profile now determines the Cord's required storage extent;
a mathematical producer cannot use the smaller rational ceiling. Existing
consumer-profile compatibility remains conservative. Lowering dispatches Quantity
values through their selected profile, including explicit v3 refusals on legacy
ports. Optional output storage covers the full producer extent; an input may
retain a smaller sealed checked-contract byte domain independently of its codec.

A sealed lowering fixture retains the mathematical profile and physical Kind,
rejects undersized output, and checks rational range admission, unsupported
symbolic order, old-profile rejection, byte limits and recovery. Its constraint
is bound in both the placement semantic law and the Fore. The initial fixture
omitted that law and correctly failed verification; the corrected fixture and
original diagnostic are retained. These are Plan/lowering boundary tests, not
Source-authored mathematical Quantity or installed kernel execution proof.

The sealed fixture also executes its Carry through the production fixed kernel.
An exact converted Pi-centimetre frame crosses ingress and egress unchanged;
post-delivery enclosure keeps its Unit/family/role. Output pressure leaves the
next input queued, repeated offers retain identity, full ingress refuses growth,
delivery frees values, and cancellation frees pending input and rejects late
admission. Prepared conversion and enclosure precede warm execution; the guarded
kernel/validation/enclosure path observes zero allocations. The first scheduler
fixture declared only one slot for two cords and correctly refused `InvalidPlan`;
its corrected two-slot declaration passes. This is real kernel transport with a
named test Carry, not an installed Std mathematical Back or authored Source proof.

## Exact typed trig consumption

The mathematical Quantity writer now reuses the existing turn-based exact trig
writer for rational turn coordinates and supported rational multiples of Pi in
radian-origin Units. A bounded structural recognizer follows only rational
multiplication/division around Pi (or exact zero); unknown shapes remain unknown.
The existing Unit conversion supplies the radian scale before the explicit 2-Pi
origin law recovers turns. This is finite exact recognition, not a universal CAS,
interval estimate, machine approximation or IEEE operation witness.

The operation checks the Angle family explicitly. SI dimension one does not
turn a dimensionless Scalar into an Angle. Tests retain original checked source
receipts for 0.25turn and 90 degrees, transport their identical Pi/2-radian Angle,
and obtain exact sine 1, cosine 0 and the truthful tangent-pole refusal. Scaled
radian and direct Pi/2-division forms work; non-cardinal rational turns reuse the
existing symbolic trig result. Unsupported symbolic/rational radian shapes and
exhausted work refuse, followed by successful recovery, with zero guarded
allocations. The private Source continuation below retains typed Angle data;
released `math/radians` continues returning its existing mathematical coordinate Type.

## Checked Source Angle continuation

A private proposed `math/angle-radians` intrinsic preserves the existing Angle
Type while applying the existing exact turn/radian law. The released
`math/radians` coordinate-extraction API is unchanged. An explicit
`new_with_mathematical_angles` preparation entrance admits primitive Angle/math
ports, native-to-symbolic-radian conversion, transport, typed trig and bounded
conditional structure. Existing profiles refuse the new conversion before
execution. Quantity arithmetic and refinements remain preparation refusals until
reviewed. The explicit private installed offer is described below.

The checked Source test compiles separate conversion, Angle carry and sine plots,
plus a nested equivalent. Original native coordinates are compared against the
existing exact reference receipt; transported radian frames retain family/role
and their canonical coordinate bytes. Quarter-turn and 90-degree inputs yield
exact sine 1; half-turn yields zero. Repeated execution observes zero allocations
and stable output capacities. Legacy native/rational and proposed mathematical
forms consume through the explicitly selected profile; old profiles refuse v3.
The optional Angle guard rejects scalar-family Units even at SI dimension one.
The initial test used an unavailable Type accessor; its diagnostic is retained,
and direct checked Type equality is used instead. Later sections cover installed
offer recomputation and actual authored Plan/kernel/CLI proof. General projection
custody and public gates remain. The spelling and form remain private proposals under #5392.

The prepared Angle profile now exposes node/depth, retained payload ceiling and
conservative aggregate limb-work bounds. Accounting includes coordinate scratch,
physical-frame output, trig output, evaluator output and prepared tree storage.
Tests require coverage of every independently prepared intermediate buffer and
stable bounds across the conversion/carry/trig chain. The work ceiling reserves
the bridge's two metered phases plus node/input validation; it is not CPU time or
WCET. Stack, allocator bookkeeping and kernel/caller storage remain separate.
The existing exact-profile bounds entrance retains its profile distinction.


## Explicit installed Std Angle profile

The private `std/pure-expression-mathematical-angle-hosted@1` offer selects this
prepared evaluator through the ordinary installed Std factory and admitted Host
Call boundary. Angle ports explicitly grant the mathematical wire profile.
The offer retains node/depth, prepared payload, numerical work, input extent and
output capacity bounds; factory preparation independently recomputes those
properties, the implementation source identity, ports, limits and Host Calls.
That source identity is a selected-module digest, not a complete executable or
live execution receipt. This initial explicit-offer checkpoint does not by itself
prove automatic Source advertisement or CLI selection; see the continuation below.

Three installed tests pass: separate checked Source conversion and sine Plans
inspect the actual transported Pi/2-radian frame; one checked Source Plan runs
conversion, Angle carry and sine with both quarter-turn and 90-degree inputs;
and factory tests reject changed profile, wire grant, all six structured bounds,
artifact identity, limits and Host Call metadata. The execution tests use the
production fixed kernel and observe zero reported post-Play-start allocations
and unchanged value capacity. The test advertisement explicitly adds the genuine
offer; this does not prove automatic discovery or product CLI admission.

The first execution correctly refused a Back requesting the old generic structured-value
input allowance against the offer's smaller sealed Host Call bound. Preparation
now uses the selected bound for this profile. The corrected tests pass; the
original refusal and a subsequent test compilation diagnostic are retained.
General projection Source/Plan/Sign custody, broader Quantity expressions,
refinements, public Candidate and exact-source integration/publication remain
separate gates. No parent acceptance checkbox changes from this private proof.


## Automatic Source Angle alternatives (private development)

Ordinary `prepare_expression_realizations` now offers the supported mathematical
Angle profile alongside existing native/exact alternatives. The finite planner
selects compatible wire profiles across the graph. Every generated alternative
counts against the existing 32-offer preparation bound, uses a distinct identity,
and is removed when replacing that Source batch. Reference annotation validation
still precedes admission. An installed checked-Source conversion/carry/sine test
passes without manually inserting offers, including idempotent preparation and
zero reported allocations after Play starts. Eleven Source preparation regression
tests pass, including native alternative retention, idempotence, replacement,
active-reservation fencing and finite offer-limit refusal without partial mutation.
The actual CLI test selects three mathematical stages, emits exact sine one,
checks sealed Source/Plan identity and Plan/Play-bound observations, and reaches
terminal completion. Both 90-degree and equivalent quarter-turn CLI runs pass, each selecting the
conversion and carry as Angle-to-Angle stages and sine as Angle-to-mathematical
consumption. Strict all-target Clippy for Std Host, Std offers and CLI passes.
Publication and general projection custody remain unverified.

## Dynamic mathematical Quantity to certified binary32 (private Core)

`PreparedMathematicalQuantityBinary32` prepares the existing bounded borrowed
Quantity enclosure evaluator, then accepts the actual symbolic Quantity frame
at execution. The selected strategy is explicitly
`numeric/enclosed-midpoint-binary32@1`: enclose the original coordinate, round
its rational midpoint under the selected binary32 rounding mode, and compute
the larger exact distance from that represented number to either endpoint.
The resulting bound must satisfy the caller's physical error-distance limit.
This certifies absolute error; it does not claim correctly rounded transcendental
results or represent a whole floating-point expression algorithm.

The receipt retains the complete original symbolic frame, its enclosure, the
separate midpoint rounding receipt, target Unit/role, and a certified error bound.
Unit conversion remains a separate operation. Point coordinates require a Delta
error distance in the same Unit; Unit changes are refused rather than performed
by the numerical projector. Source and target frames retain their family and
dimension through their checked Units. Exact point intervals use exact-distance
claims, while nonzero enclosure widths retain certified-bound claims.

The existing general projection report records numerical precision loss,
representation transformation by the named strategy, and physical-coordinate
preservation. It rejects false exactness, missing physical obligations, changed
policy/rounding, a changed numeric target, and a changed Unit even with identical
float bits. No new numerical-only report mechanism is introduced.

Three new tests pass, covering radian, Fahrenheit point and Fahrenheit Delta
coordinates; twelve repeated guarded dynamic executions allocate zero times and
retain prepared payload capacity. Tests independently check both endpoint
distances, strict zero-tolerance refusal, zero caller work, impossible zero-work
preparation, retained-storage refusal and successful recovery. Fourteen existing
binary32, physical enclosure, coordinate report and frame regressions pass.
Core all-target strict Clippy and no-default-features checks pass. An initial
regression command named a nonexistent test target; the corrected suite ran.
A test initially assumed zero-work preparation succeeded; it now checks its
existing truthful preparation refusal.

This is Core computation/projection evidence. The installed input-consuming
Back, selected offer/Plan bounds, independent serialized-result verification,
actual CLI and Source/Plan/Sign custody still need integration. The original
parent scope and public acceptance gates remain open.


## Symbolic Near result integration (private, verification in progress)

The existing Near result schema now retains the full mathematical Quantity frame
in `source-wire` (33,829 bytes) and explicitly names the new midpoint strategy as
`enclosed_binary32`. Its result envelope is 131,072 bytes, within the existing
262,144-byte structured-value ceiling. The widened Kind contract is explicitly
`numeric/quantity-near@2`; direct Fore aliases must declare its current byte
bounds. The planner continues to require equal Fore/Back contracts. Eight old
fixture declarations initially failed those strict checks (one bound assertion
also retained 16,384); current development fixtures are being migrated. Old
schema bytes and historical proof receipts are not rewritten.

The new installed profile reuses the Near lifecycle and independently recomputes
its offer bounds and artifact identity. Its prepared producer retains the exact
symbolic coordinate, complete physical source wire, Unit/role, request, effective
allowance, actual error bound, rounding and selected strategy. The ordinary
rational/native profiles still use their exact wire grant and old algorithms.
The existing reader continues to refuse mathematical representation grants.

`PreparedMathematicalNearQuantityVerifier` pins its schema, enclosure parameters,
rounding, consuming physical limit and finite work ceiling before execution.
It reserves generic structured-validation work, replays the projection from the
original physical frame, and checks the emitted target, claimed error, duplicated
source fields and optional original-wire digest. It does not attest supplier
execution. Two producer/reader tests pass, including canonical target/error/
source/Unit forgery, digest mismatch, exhausted work, stricter consuming tolerance,
recovery, repeated zero-allocation execution and unchanged prepared capacities.
Installed authored-Plan/kernel proof, current-contract regression, final lint and
CLI/public acceptance are still pending at this checkpoint.


The current Near regression now passes 22 tests (including the two new
producer/reader tests), with one pre-existing ignored test. A third-party native
codec input explicitly selecting the mathematical profile follows the original
rational algorithm and retains its original wire; both representation payload
composers are prepared in advance. Default native selection keeps the existing
binary32/exact alternatives ahead of the mathematical alternative. Zero-limit
mathematical inputs remain eligible for exact point outcomes and refuse actual
nonzero approximation loss.

The first authored Angle bridge/carry/Near chain correctly fails canonical
expansion with `CND-FRM-045`: a typed Angle cord is not a generic Quantity cord.
No connection check was weakened and no successful installed dimensioned-chain
claim follows from the component proof. A reviewed explicit family-to-generic
physical value bridge is still needed; it must retain original wire, family,
dimension and role and have bounded prepared execution. The candidate end-to-end
test remains pending in this private checkout. Final lint/public gates remain
pending. The small content-identity helper was extracted from Near lifecycle
code to keep production responsibilities under 500 lines; historical identity
construction is unchanged and current Near regressions pass. Full workspace
checks remain unrun locally under current disk pressure; later combined public
integration remains required.


## Explicit physical carrier and installed dimensioned chain

The private `quantity/value` operation explicitly exposes an already checked
physical value through the generic Quantity carrier. It forwards the complete
wire unchanged: it creates no Unit, converts no coordinates and changes no role.
Bare scalars, mathematical data and Unit values refuse during canonical checking.
Old prepared profiles refuse the operation; the finite mathematical physical
profile prepares its child and result storage before execution. Its existing
Angle transport/trig subset now admits this carrier operation and generic
physical primitive ports, while arithmetic/refinements remain separate work.

Two conformance tests pass. Native, rational and symbolic Angle frames survive
25 guarded successful executions byte for byte, with one wrong-family refusal
followed by recovery, zero allocation/reallocation and stable prepared capacities.
An initial borrowing diagnostic was corrected by consuming the existing borrowed
value view. The first negative test stopped at syntax-document checking; the
corrected test reaches canonical expression checking. The constructor's initial
Angle/math-only port restriction was explicitly extended for physical carriers.

The authored installed test now passes for both 90 degrees and quarter-turn:
Angle-to-radian conversion, typed Angle carry, explicit carrier exposure, then
the selected mathematical Near binary32 consumer. The independent reader checks
the resulting original Pi/2-radian frame, Unit/role and certified nonzero numeric
error. Both actual kernel runs report zero post-Play-start allocations and
unchanged value capacity; the separate guarded reader also allocates zero times.
All five installed Angle tests pass. This does not yet establish full generic
projection Source/Plan/Sign custody or complete original-scope acceptance.

The first installed rebuild terminated with ENOSPC before tests. Protected source
and existing evidence were retained; inactive owned old check metadata and older
Conduit library-cache variants were removed, then the same test passed after a
fresh build. Full workspace checks remain unrun locally under disk pressure.
Strict all-target Clippy for Core, Plot, Std offers, semantic catalog and Std Host
passes, as do Core/Plot no-default-features and formatting checks. Actual CLI
proof remains pending. Existing public gates and all original parent requirements
remain authoritative.
