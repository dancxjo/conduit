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

## Actual CLI dimensioned binary32 continuation

The CLI now executes the same native Angle, exact radian conversion, typed carry,
explicit Quantity carrier and input-consuming binary32 projection. Both 90-degree
and quarter-turn runs select three mathematical expression stages and the
`std/quantity-near-enclosed-midpoint-binary32@1` consumer under the version-2 Near
contract. Authored inspection checks the exact Pi/2 coordinate, radian Unit,
reported error and requested allowance before emitting its success text.

The error inspection uses an exact rational golden for this selected enclosure
profile and input. It was captured from the Core certificate, with target bits
checked as `0x3fc90fdb` and the positive error checked below the requested
one-millionth-radian allowance; offline canonical-rational decoding records its
Source literal. This is transport/selection conformance, not an independent new
mathematical oracle. Core enclosure and endpoint-distance tests retain their
separate numerical proof. The first fixture tried ordinary ordering operators
on MathematicalValue; the actual Source API deliberately refuses that operator,
so the fixture uses exact equality without inventing implicit symbolic order.

Two CLI tests pass: two successful equivalent-angle runs and one zero-allowance
run that returns InvalidInput/detail 3 without success output. Successful reports
retain one verified Source-bound Plan, the expected selected profiles and spans,
a completed terminal, and observations bound to that actual reported Play and
Plan. Focused CLI strict Clippy passes. This does not prove general projection
report-to-Sign custody, arbitrary symbolic Quantity arithmetic, qualified whole
floating-point computation, or public acceptance; all original parent checks
remain open where previously unaccepted.

### Private all-form Angle conversion continuation

The prepared `math/angle-radians` path now accepts each representation admitted
by its mathematical Angle port: native decimal, exact rational, and bounded
mathematical coordinates. Rational inputs retain the existing canonical
radian denotation. Same-origin radian scales remain exact; turn-origin inputs
use the explicit `TURN_RADIAN_REFERENCE_LAW` and retain Pi symbolically.
An already-radian mathematical frame is preserved byte for byte. Preparation
owns builder and coordinate storage, and execution clamps its work meter.

Focused Core tests cover all three forms, same-origin symbolic milliradians,
identity conversion, output atomicity on refusal and recovery. A checked Plot
chain divides a native Angle through the exact profile, converts its rational
output twice through the mathematical profile, then obtains exact sine one.
The guarded executions report zero allocations and unchanged retained bounds.
This is private component and checked-expression evidence. Installed Host/CLI
chains for this extension, qualified consumer custody, and stable publication
remain unproven; the original parent issue stays open.

The CLI fixture now includes two separate Angle stages, halving then doubling,
before symbolic conversion. Both stages must select the exact Quantity profile;
the original three mathematical stages and enclosed binary32 boundary remain
selected with Source spans. The two CLI tests pass for 90 degrees and a quarter
turn, including the unchanged exact certified-bound check and zero-allowance
refusal. This supersedes the missing CLI proof noted above for this extension.
The extended installed kernel allocation and independent reader tests are
pending; general custody and stable acceptance remain separate requirements.

The extended installed tests now pass (six tests). The transformed dimensioned
chain selects six stages, delivers its certified binary32 record, and replays
that record with the independent mathematical verifier. Both native spellings
retain Pi/2 radians; kernel post-Play allocation counts and independent reader
allocation counts are zero, with unchanged kernel value-storage capacity. A
separate five-stage exact transform/conversion/carry/sine chain returns exact
one. This supersedes the pending installed-test note above. The retained proof
remains private; qualified consumer integration, general projection/Sign custody,
and exact-source stable acceptance remain open.

### Private prepared symbolic consuming contract

Core now exposes an explicit `PreparedMathematicalNearValueContract`. Preparation
validates and retains the entire authored checked contract, then pins enclosure,
rounding, consumer tolerance, replay storage and a finite numerical work ceiling.
Execution checks its input extent and every authored predicate, including the
symbolic certificate replay. The ordinary checked-contract validator is unchanged.
This new entrance currently consumes mathematical source frames; native/rational
compatibility dispatch is required before integrating a general hosted consumer.

One real-producer test passes across four guarded cycles: valid replay, forged
Unit rejection, tighter-tolerance rejection, independent byte-length rejection,
zero-work refusal, prepared work-ceiling enforcement and refusal recovery. The
contract remains identical and verifier buffers remain fixed, with zero guarded
allocations. Core strict Clippy and no-default-features checks pass. The executed
Std test binary and source bundle are retained with verified archive roundtrips.
This is prepared Core contract evidence, not qualified Source/Plan/kernel
consumer admission. Explicit evaluator/Host resources and artifact binding,
native/rational dispatch, general projection/Sign custody and stable acceptance
remain required; no original parent requirement is closed by this checkpoint.

### Private checked Source consumer replay across admitted forms

The prepared consuming contract now dispatches by the validated source wire
version. Native and rational records retain the existing exact verifier;
mathematical records use prepared symbolic replay. Both paths share one clamped
meter and one canonical frame/schema validation. A failed proof does not select
a fallback verifier. The ordinary unprepared validator remains unchanged.

The explicit checked-expression constructor
`new_with_mathematical_near_contracts` prepares qualifying input and output
contracts before execution. Its bounds include additional replay containers and
buffers and the sum of predicate replay meters. These are component bounds,
excluding contract-owned heap payload, allocator overhead and general evaluator
record validation; Host admission must account for those separately.

All eleven focused Near tests pass on the final source. A checked Source
`Angle near 0.000001rad` consumer accepts independently produced native,
rational and symbolic records, rejects a forged Unit, and preserves exact output
bytes. A stricter output tolerance rejects Pi/2, then accepts an exact native
record on the next evaluation. Four guarded cycles have zero allocations,
fixed output capacity and unchanged reported bounds. The existing Core evidence
regression also confirms rational extraction exhaustion remains WorkLimit.
Strict Core/Plot library Clippy and no-default-features checks pass. Source
bundles and both executed test binaries are retained with verified hash/archive
roundtrips.

This supersedes the missing native/rational dispatch and checked-evaluator
integration noted above. An installed qualified Host offer still needs exact
resource/property and artifact binding, including general structured validation
work, factory recomputation, Source/Plan/kernel/CLI proof and stable acceptance.
General projection/Sign custody and all remaining original requirements stay
open; this checkpoint does not establish full parent completion.

### Private installed qualified consumer and actual CLI

The explicit hosted profile `std/pure-expression-near-contract-hosted@1` now
prepares consuming predicate replay before Play. Source batching offers it for
`near`-qualified expressions, retains custom offers, and replaces its own prior
batch idempotently. Its factory rechecks the existing annotation-witness gate,
exact Fore laws, implementation/module identity, replay/component bounds, Host
Call extents and queue limits. Changed or missing facts refuse installation.
The profile retains Value, Flow, closing Flow and Current offer contracts;
actual execution proof here remains the Value path.

Its sealed property separates predicate replay meters and prepared replay
containers/buffers from per-record mathematical validation and structural
node/depth/extent caps. These are finite component bounds, not a total CPU/WCET
or allocator/kernel/stack memory claim. The module-source artifact identifies
reviewed code; separately retained binary hashes identify the executables used
for this proof. Neither alone attests a later live execution.

Six installed Angle tests, eight qualified Fore tests, and eleven Source-offer
lifecycle tests pass. The dimensioned chain has two exact arithmetic stages,
symbolic radian conversion, carry, explicit physical exposure, selected binary32
projection and a qualified consumer. The outer Fore retains the consuming
tolerance; its earlier planning refusal is preserved rather than bypassed.
Actual kernel execution reports zero allocations after Play starts and fixed
value-storage capacity, followed by independent certificate replay.

Two CLI tests pass across three invocations: equivalent 90-degree and quarter
turn runs select the prepared consumer and inspect the exact certified-bound
value, while zero allowance refuses without success output. One additional real
CLI run retains stdout and its raw report, with matching actual Source/Plan/Play
bindings, selected profiles and completed terminal. All-target Std Offers/Host
Clippy and focused CLI test Clippy pass on the final implementation. Source
bundles and three executed binaries have verified archive/hash roundtrips.

This supersedes the missing installed consumer and CLI proof noted above. The
raw report has only a terminal observation, with no retention drops. The
retained kernel events identify node/port/request, but not the numerical report
content digest. Per-projection general report/Sign custody is therefore still
unproven and must use the shared projection/Sign system. Total cost/latency,
full resource accounting, broader dimensioned and whole-floating-algorithm
requirements and exact-source stable acceptance also remain open. No original
parent checkbox is closed by this private checkpoint.

### Private kernel content binding foundation

The existing Sign sink now has a routing-content entrance. Its default retains
exactly the ordinary routing event; it adds no content claim to legacy profiles.
An optional `ContentSignLog` binds the actual stored bytes and exact Cord to the
same retained event sequence. The kernel takes no dependency on Core or a crypto
library. The admitted owning profile must identify and bound its allocation-free
digest provider; the current conformance provider is explicitly noncryptographic.

Preparation reserves a fixed binding array large enough for every retained Sign
in the underlying fixed or hosted log. Each routing binding expires with its
actual transient event under the shared oldest-transient eviction/gap rule.
Pinned terminal/remote evidence remains intact. The separate binding byte budget
covers its exact backing array; complete inline container size is available, while
provider/inner heap payload, stack and allocator costs remain separately owned.
This invariant prevents content storage from introducing a new late capacity
refusal after an otherwise admitted routing commit.

Payload, normal-close, abnormal and quiescence routes pass their actual stored
bytes at the existing commit boundary. The debugger wrapper forwards this
mandatory entrance and the underlying capacity admission; debugger pressure or
detachment does not own or erase the content evidence. No scheduler, emission
policy, value ontology or KernelEvent layout is duplicated.

All 254 kernel tests pass with allocation support (six pre-existing manual tests
remain ignored), including eight new content tests. The actual scheduler retains
content after value storage is released; queue pressure and cancellation do not
bind an uncommitted second output. Fixed/hosted histories agree, transient gaps
expire the correct bindings, pinned remote identities survive, and guarded tests
report zero allocations/reallocations. Strict all-target kernel Clippy and the
no-default-features library check pass. Twenty-seven executed test binaries and
the source bundle are retained with verified archive/hash roundtrips.

This is kernel foundation evidence. Std does not yet select this content profile,
install a declared digest provider, admit its full storage/hash costs, retain its
bindings in product artifacts, or join a general projection report to actual
Source/Plan/Play/Sign identities. Input/request/output custody for whole numeric
algorithms, target proof, total cost and stable acceptance also remain open.
The earlier terminal-only CLI report remains the current product evidence;
this checkpoint does not retroactively strengthen it or close a parent item.

### Private Std admission of routed-content evidence

The explicit qualified Std consumer now declares the shared
`std/routed-content-signs@1` requirement. Installation checks its exact digest,
storage and work policy before Play; missing required or changed policy refuses.
Undeclared profiles retain their ordinary Sign behavior. The selected provider
uses Core semantic SHA-256 with domain `conduit/routed-value-content@1` and scheme
`core/semantic-sha256@1`. Three independent Python SHA-256 reference vectors
verify the declared domain and exact framing, including empty and binary input.

Hosted binding storage is allocated once before Play at the underlying retained
event ceiling. Its exact backing bytes and maximum digest input extent are
retained in the Std kernel report alongside native content bindings. Digest work
is one bounded-input hash per committed route; this is not a total lifetime work
limit or a CPU/WCET claim. Standalone, Body and remote installation use the same
wrapper. Product and Body artifact export remain separate unfinished work.

Six installed Angle tests join the selected numerical producer's actual output
bytes, their independently recomputed digest, retained native ValueRouted event,
and lowered Cord/Connection identity. They retain the existing zero-allocation
Play guard and independent numerical certificate reader. Two admission-policy
and eight qualified Fore tests pass. The full Std library suite passes 867 tests
with 18 existing ignored tests; the full kernel suite passes 255 with six existing
ignored tests. Kernel no-default and strict Clippy checks pass. Workspace
all-target Clippy with warnings denied passes. The first full workspace test
invocation stopped during linking with an explicit disk-full IO error and a
linker bus error; it provides no workspace test pass. Its diagnostic log is
retained while inactive owned build outputs are recovered for a continuation.
Twenty-eight executed Host/kernel binaries and the
independent digest test binary have verified archive/hash roundtrips.

This advances native execution evidence beyond the earlier foundation. It does
not yet establish a Core SignId for each retained route or general projection
report/input/request/output custody. Current product export still retains Core
observations rather than these native bindings, and its earlier CLI proof remains
terminal-only. The execution identity map currently reserves only presenter and
terminal Sign capacity and associates node-specific Signs with Host requests;
ordinary routing evidence needs an explicit, bounded shared identity contract.
Do not relabel a constructed digest witness as a Gear-produced ValuePayload.
Broader original requirements and exact-source stable acceptance remain open;
this checkpoint closes no parent audit item.

### Private shared routed-content Sign export

Core observation definitions and Std observation projection are extracted into
responsibility-owned modules, retaining existing public Core names. The new
shared `RoutedContent` observation is appended after existing wire variants.
It carries digest domain/scheme, byte extent, native event sequence and exact
output port, while the enclosing observation retains Plan/Play, Host/boot,
producer placement and connection identities. It is retained routing evidence,
not a constructed Gear-produced value or a general projection certificate.

The optional Std content profile reserves Core Sign identity capacity before
Play at its native retained-event ceiling. After Play, internal routing rows
must join retained ValueRouted events and the exact planned Cord source before
receiving canonical Core Sign identities. External Fore boundary rows remain
native evidence; this checkpoint does not invent producer placements or internal
Cords for them. Sign artifact validation additionally checks source Host/boot,
port, connection extent and finite digest labels against its exact Plan/Play
context. It validates structure and correlation, not hostile-code attestation.

All six installed Angle tests pass with the new actual numerical output-to-Core
Sign join, existing independent numerical replay, and zero-allocation Play
checks. The same preserved test executable passes the complete Std library suite:
867 passed, 18 existing ignored. Four artifact-envelope tests pass in their first
form; expanded extent/label refusal cases, wire compatibility, actual CLI export
and strict workspace checks are still running or queued at this checkpoint.
The executed artifact and installed test binaries have verified gzip/hash
roundtrips. Full workspace tests remain unproven after two baseline link-time
disk exhaustion failures; their logs and cleanup receipts are retained.

This advances shared internal routing custody. Native retention-gap export,
external Fore/Body/remote product evidence, canonical general projection reports,
and whole-algorithm source/input/request/output custody remain unfinished.
The earlier actual CLI report remains terminal-only evidence until the new real
CLI gate passes. Public stable acceptance and the broader original parent scope
remain open; no parent requirement is closed here.

### Private actual product routing artifacts

The real dimensioned CLI gate passes both degree and quarter-turn input runs,
requiring the selected binary32 producer's routed-content Sign in the ordinary
report. The third invocation retains the zero-allowance refusal. Two independent
observation-wire tests pass, preserving previous postcard tags and roundtripping
the appended shared variant. The full Observatory library passes all 54 tests,
including the additional byte-ceiling and digest-label refusal cases.

A separate retained CLI execution exports ten routing Signs and one completed
terminal Sign, with zero report retention drops. Another actual execution writes
standalone Plan, Play and all eleven Sign artifacts through the existing product
writer. Ordinary `conduit inspect` validates every artifact and the authored
Source. Independent SHA-256 reconstruction reproduces every canonical Sign
identity; each routing observation names its exact planned producer/output/Cord.
The selected projection's Core Sign sequence is 8, while its native routing event
sequence is 76; these identities are deliberately distinct. Its transported
record extent is 4,431 bytes. Raw report, compressed verified copy, artifacts,
inspection output, executable hashes and source bundles are retained privately.

This supersedes the terminal-only product-export limitation of the earlier
checkpoint. The strengthened automated CLI artifact/inspection gate passes both tests
across three invocations, including ordinary inspection of the exact projection
Sign artifact. Workspace all-target Clippy with warnings denied and the Core
no-default-features library check also pass. Routing digests do not themselves
retain the complete numerical record and canonical general projection report;
independent numerical replay is established by the installed test, not by
pretending the digest is a certificate. Native gaps, external Fore/Body/remote
export, whole-algorithm input/request/output custody, full workspace test proof,
public stable acceptance and the broader original parent scope remain open.

### Private native execution-history gaps

The optional shared routing-evidence profile now exports an
`ExecutionEventGap` observation when transient kernel events were evicted. Its
first/last native sequences and exact evicted-entry count remain distinct from
Core Sign sequences and snapshot retention drops. The count may exclude pinned
native events inside that span. A gap names its exact owning Plan/Play/Host/boot,
without inventing a producer placement or Cord. Standalone artifact validation
refuses malformed ranges/counts and a foreign owning Host/boot.

Preparation reserves one additional Core Sign identity slot beyond the native
retained-event ceiling. Export retains the gap even after every routed-content
row has expired. Existing observation wire tags remain unchanged; the new gap
has its own appended tag. Three wire tests and all 55 Observatory tests pass.
The bounded log/export contract test retains a gap and a real retained routing
row with canonical Core Sign identities, then evicts that last row and still
exports the gap. Its guarded log operations allocate zero times. Its native
bytes are explicit fixtures; this is not a new live-device or qualified Flow
execution claim. The exact executed binaries have verified archive roundtrips.

The same installed test executable passes the full Host library suite: 868
passed and 18 existing ignored tests. Actual CLI artifact/inspection regressions pass both tests across three
invocations, and workspace all-target Clippy passes with warnings denied. The
Core no-default-features library check also passes. Full workspace test
proof remains missing after the retained baseline disk failures. Broader external
Fore/Body/remote export, complete general projection-report transport, exact
policy/request configuration custody, whole-algorithm input/output custody and
public stable acceptance remain open. No parent requirement is closed here.


### Private complete mathematical receipt transport checkpoint

The mathematical binary32 domain now implements the shared general projection
codec. It preserves the complete physical symbolic frame, enclosure endpoints
and width, all enclosure requirements and work metadata, the midpoint's full
physical and numerical rounding receipt, exact error claim, and complete
allowance/rounding configuration. It does not substitute a summary or source
hash for these values. The codec identity fixes its field order and tags.
Independent comparison requires replay with the same admitted profile and work
budget; matching target bits and error alone are insufficient.

The isolated Core five-test suite passes, including sixteen source/rounding
combinations across exact zero, symbolic radians, affine points and deltas.
Replay, encoding, verification, every-byte mutation rejection and configuration
mismatch checks execute with zero allocations. Strict Core all-target Clippy and
Core no-default-features checks pass. The earlier complete workspace Clippy run
passes for the generic encoder before this domain-codec addition. Source and the
executed test ELF are retained with verified hash/archive roundtrips.

This remains private preparation. Near does not yet transport this general
report or reconstruct it in its independent reader. Exact request/certificate
binding, product artifacts and inspection, profile work/storage admission,
broader regression execution and stable acceptance remain open. No original
parent audit requirement changes status from this component proof.


The prepared mathematical projection now also supports replay under an original
producer allowance while separately enforcing the prepared consumer allowance.
It retains the producer allowance in both the full receipt and midpoint receipt;
a reader cannot silently substitute its own tolerance into the transported
policy. A tighter or looser consuming limit reproduces identical general report
bytes when the actual certified error is acceptable to both. A zero-error reader
still refuses a lossy receipt. Negative, physically incompatible and insufficient
producer allowances refuse, and prepared scratch remains reusable afterward.
The ordinary producer path keeps its existing single tolerance comparison.

Six focused Core tests, strict Core all-target Clippy and Core no-default checks
pass for this prerequisite. The new replay, refusal and recovery checks perform
zero allocations. This API is not yet wired into the Near record reader; the
wire integration must pin its original projection work budget as well as the
producer allowance before comparing complete receipt bytes. Parent audit
statuses and public admission remain unchanged.


### Private Near reader original-policy reconstruction

The actual mathematical Near reader now reconstructs the original requested
allowance before numerical replay. Producer and reader share the existing
200-million projection work ceiling; envelope parsing and physical checks cannot
silently change the replay receipt's admitted budget. Insufficient remaining
work refuses before replay, and failed replay work is charged without fallback.
The verified result retains the complete independently replayed receipt, with
its original allowance, enclosure profile, work metadata and midpoint rounding.
Existing source/target/rounding accessors keep their meaning.

The domain codec exposes derived bounds: source 50,431 bytes, target 777, typed
error detail 1,035 and policy 1,808. These cover complete canonical field data;
they are not a total Near envelope, CPU, stack or whole-Host memory bound.
The returned full receipt remains caller-owned storage requiring explicit
profile accounting alongside codec work and transport buffers.

Seven Core tests pass, including actual canonical Near record verification
that reproduces the producer's entire general report under both tighter and
looser accepting consumer limits. Zero-error consumers, altered requests and
insufficient replay budgets refuse, and recovery succeeds with zero observed
allocations. Strict Core all-target Clippy and Core no-default checks pass.
The integrated four Std mathematical Near tests pass, followed by the complete
Std library suite: 868 passed and 18 existing ignored tests. Exact Core and Std
executed binaries and source bundles have verified hash/archive roundtrips.

The Near wire record still does not retain the generic report bytes, and the
outer certificate/request binding, ordinary inspection, product artifacts,
complete finite admission and stable acceptance remain unfinished. The original
93-requirement audit text was rechecked unchanged; no status is closed here.


### Private completed mathematical Near report wire transport

Completed `enclosed_binary32` Near records now carry the full shared general
projection evidence bytes. Kind revision `numeric/quantity-near@3` and the
explicit Std mathematical back profile at revision 2 identify the changed wire
contract. Native/rational representation variants retain their existing value
behavior; this checkpoint does not yet transport their general reports.

A shared Core content-identity helper binds the exact source wire, target and
original requested allowance. The producer encodes the existing admitted
computational report into a fixed 54,650-byte staging buffer. The prepared leaf,
floating record and representation composers account for their added storage;
the full maximum mathematical record is checked against the unchanged
131,072-byte output envelope before Play. The bound includes all maximum domain
payloads and actual canonical schema bytes, rather than observed small fixtures.
Selected-offer bounds include the maximum evidence extent and three field
encoding passes. Producer/consumer module-source artifacts now include the new
helper, codec and shared general evidence implementation. These identities remain
source correlations, not executable or execution attestations.

The independent reader replays the original producer policy with the pinned
projection budget, checks every certificate field against that exact receipt,
then reconstructs and compares every report byte. A larger sufficient error
claim is refused because it changes the original certificate. Changed requests,
report summaries, unsupported versions, truncation and trailing report bytes
refuse without allocating; a valid record remains verifiable afterward. The
report is computational evidence, with execution custody supplied separately by
the actual kernel's emitted-value Signs.

Seven Core tests, strict Core all-target Clippy, Core no-default checks and
workspace formatting pass. Five focused Std tests and the full Std library suite
pass: 869 passed with 18 existing ignored. The real CLI dimensioned-chain suite
passes two tests covering three invocations, including ordinary artifact/Sign
correlation and a zero-allowance refusal. Exact executed Core, Std, CLI-test and
CLI binaries have verified SHA-256/archive roundtrips; code source bundles and
file hashes are retained. The first compile exposed the new refusal's exhaustive
Plot mapping, which now has its own machine-readable code. Workspace all-target
Clippy with warnings denied also passes. Full workspace test execution still
lacks the previously documented multi-gigabyte linker headroom.

Raw report persistence in product artifacts, ordinary generic report inspection,
remaining profile accounting, native/rational report transport, broader original
requirements and exact-source stable acceptance remain open. No parent audit
status changes from this private component proof.


A separately retained ordinary CLI run completes the same dimensioned chain.
All thirteen Plan/Play/Sign artifacts and the authored Source pass ordinary
inspection. Independent SHA-256 reconstruction verifies all eleven canonical
Sign identities and their actual Plan/Play/connection context; ten are routed
content observations, and retention reports zero drops. The mathematical
projection's actual routed value is 11,436 bytes under the revision-2 back.
The raw run report, inspected artifacts and Source are retained, including a
verified compressed-report roundtrip. The Sign records the digest and extent;
it does not itself preserve the original report-containing value bytes. That
remaining product custody gap is not replaced by this correlation proof.


### Private bounded routed-value custody prerequisite

The shared kernel content sidecar can now opt into caller-provided fixed value
storage before Play. It reserves exactly one maximum-sized byte slot per binding
row, with checked address-space/u32 multiplication and no allocation or growth
on routing. Only the concrete contiguous native-event logs implement the stronger
retention contract needed for collision-free slots. Retained value bytes remain
available after value-store consumption; expired bindings expose no stale bytes.
The existing digest-only profile retains its behavior and captures no raw data.

Preparation validates every admitted Cord value extent against the capture
ceiling in both scheduler constructors. Oversized custody configurations refuse
before scheduling, including through the debugger wrapper; direct oversized
recording refuses before hashing or event mutation. The Sign interface and exact
plan-admission responsibility were extracted into focused modules while keeping
public paths. Std forwarding and module-source identity inputs include the new
admission/storage collaborators.

Sixteen focused tests, thirteen fixed/no-default tests and the complete Kernel
suite pass: 262 passed with six existing ignored tests. Strict Kernel all-target
Clippy, Kernel no-default library checking, workspace formatting and workspace
all-target Clippy pass. Actual scheduler pressure, consumption and cancellation
checks observe zero allocations and no uncommitted captured output. All 28
executed Kernel binaries have verified archive/SHA-256 roundtrips, and source
bundles are retained. This is bounded native byte custody, not semantic validation
of arbitrary values, authenticated execution or device proof.

Std still selects digest-only capture. An explicit value-custody profile, its
full storage/work admission, product value artifacts and ordinary generic report
inspection remain unfinished. This prerequisite closes no original parent audit
requirement and has no stable-acceptance claim.

### Explicit Std value custody and product artifacts (private development)

Std now admits raw routed bytes only under the exact
`std/routed-content-values@1` realization property, together with its exact
content-digest property. Preparation checks the slot-count/maximum-value byte
product, reserves the fixed arena before Play and records the mandatory copy
pass. Altered policies, missing digest admission and overflowing budgets refuse.
Plain and digest-only execution retain their existing storage behavior.
Post-Play export verifies retained bytes against every retained content binding.

The mathematical binary32 Near producer and qualified consumer request this
profile. The ordinary product artifact export retains those actual bytes in
`conduit.routed-value/artifact@1`, with the exact producing Sign and Plan/Play
context. `conduit inspect` checks context, producing port kind, extent and content
digest. It explicitly reports that domain semantics were not validated. Export
refuses absent or ambiguous routing Sign context before committing its output
directory. External Fore routes that currently lack a corresponding Core routing
Sign therefore remain unsupported for raw-value artifact export; no connection
is synthesized to conceal that limitation.

The complete Std library suite passes (871 tests, 18 ignored), six focused
Observatory artifact tests pass, and both CLI tests pass across three actual run
invocations. The positive cases cover degree/turn inputs, retained general report
bytes, ordinary value inspection and refusal of changed bytes. A separate retained
CLI run exports ten routed values. Independent Python SHA-256 and exact context
checks verify all ten, including the 11,436-byte mathematical projection record.
The source and executed binaries are preserved in the local proof custody.
Workspace formatting and strict workspace all-target Clippy pass. The full
Observatory suite passes all 56 tests; the final-source CLI suite passes again.

This establishes inspectable byte correlation for the supported routing context.
Generic report metadata/domain inspection, native/exact general report transport,
complete resource and target proof, public admission and stable acceptance remain
open. The original 93-row parent audit remains unchanged; this checkpoint closes
no parent requirement.
