# Native local implementation checkpoint

Tonight's user instruction defers proof/CI work in favor of implementation.
This branch includes current `dev` (1e056b899), the selected typography work,
and the four-line obsolete lint-expectation removal. No new PR or CI queue is
created by this checkpoint; no stable acceptance or issue closure is claimed.

## #6158 continuation

An empty authored region never reserves a lane. A sole populated panel uses
full width from its first page through continuation. After the first region of
a populated pair is exhausted, continuation of the remaining region also
occupies the available panel width. Page arrangement precedes field and
chunk measurement. A page advance remeasures wrapping, command cost and control
bounds; panel fill/outline reservations, source reading order and the existing
20-row/48-command limits remain in force. Narrow layouts retain their existing
single-column path. Focused runtime/regression and wide/narrow acceptance remain
outstanding.

## #5516 / #5518 / #5519 Patchbay

Pointer targets are collected from the traversal that emits actual Port rows,
Gear cards and eligible Cord paths. Port rows win over card backgrounds; cards
cover paths beneath them; only uncovered emitted path segments are selectable.
Six cards, 24 Port rows and 12 emitted Cords are finite reading targets, never
invented semantic actions or topology.

Tab/Shift+Tab cycles all exact Gear/Port/Cord subjects, including omitted and
off-page subjects, through the existing checked FaceNavigation FocusAndDisclose
operation. Details retains complete source facts and marks the selected subject.
Mandatory card/Port/overflow commands and shared chrome are reserved before
admitting Cord paths. Drawing, pointer targets and omission counts share the
resulting prefix, which may be less than twelve; keyboard navigation retains
all omitted Cords. Escape returns to the originating diagram page; Enter on a
reading target does not Invoke. Existing Show/Face checks and pending-draft refusal precede changes.
Native now consumes `PatchbayNavigationProjection::for_graph`, a shared
structure reader built with the existing Patchbay navigation/membership machinery.
It exposes the complete serializable navigation/projection/cursor bundle accepted
by the browser workbench and `render_linear_navigation`. Focus discloses Exact
depth, retaining endpoint IDs and provenance in those projections. Native geometry
never defines membership. Actual cross-Mask comparison and QEMU acceptance remain
outstanding.

Read-only review found no blocking defect. Formatting and compile-only checks (the shared-projection update took 6 seconds)
are the local verification scope; broad tests and product proof are deferred.

## #5629 diagram fallback and focus

When enlarged text changes measured chrome beyond the diagram's fixed furniture,
F3 selects the complete measured Details reader, retains graph navigation and
records a typed InvalidExtent diagram refusal with a short fallback notice.
A Look replacement re-resolves a selected graph subject to its new Details page;
old page offsets cannot move that subject's focus. A real action selected in
Details leaves graph-reading mode and retains normal Enter/draft behavior.
The existing expanded-chrome regression assertion now describes this fallback;
it was updated but not executed tonight. Compile-only checks pass.

## Coordination

The original `work/native-look-local` branch remains untouched. Its small
forced-color primary-focus fix (2d1c9a50c) and configured-layout fallback fix
(360ec7199) were reviewed and reused here with provenance in the new commit
messages. An unfit configured layout now uses the existing generic reader with
the same Face/expression and an explicit ConfiguredLayout refusal. Primary focus
retains an independent contrasting inset under forced colors.

The alternative Patchbay implementation on that branch is retained there;
this branch has one selection model using the shared portable projection and
same-traversal hits.

## #6190 required-item admission

The existing Source required-item component now has four expression roots in the
same finite Todo catalog. The closed `required-item` recipe passes through the
existing owner request/return codecs, exact native plan reconstruction and browser
chooser. Shift+F8 selects it without changing the current owner Look; existing
F6/F7/F8 meanings remain. Shared reverse lookup accepts only exact reviewed roots.
Draft, authenticated predecessor-Show, Host-offer, Fore and four-refresh checks are
unchanged. Real completion removes the component's required one-way action, so
the existing explicit generic fallback can be reached without a fault flag or
unreachable item-count fixture. Source/admission/runtime proof is still deferred;
no installed-owner fallback or stable acceptance is claimed from compilation.

The required-item owner binary and native compositor/owner-network code compile;
JS module syntax checks pass. An optional Source check with the retained db1 CLI
reports CND-FRM-053 on VisualExpressionInfo for both the existing todo-mask source
and the extended required-item source. That baseline diagnostic does not establish
native Host-specific admission; runtime/catalog acceptance remains deferred.

The generic Source diagnostic was traced to missing component checking definitions
in the ordinary Mask catalog, not excessive profile bounds. The catalog now
registers the existing VisualExpressionInfo/ComponentRecipeInfo schemas and
component kind, without any Host Back/offer/grant. A rebuilt local conduit CLI
reports no diagnostics for both todo-mask.conduit and todo-required-item.conduit.
A focused catalog regression was added but not run tonight. This resolves the
Source-checking defect; installed-owner runtime fallback proof stays deferred.

## Runnable local image

The normal `cargo +stable xtask make conduitos live x86_64` build completed
successfully at source c135ca3eb. The local ISO is
`target/conduitos/live/x86_64-pc/conduitos-x86_64.iso` in this worktree.
No QEMU journey, acceptance suite or CI job was launched. Interactive
`cargo +stable xtask make conduitos live-boot x86_64` requires a graphical display;
this shell has none. The image build establishes compilation and packaging only.

## Shared graph provenance correction

The single-aspect graph projection now explicitly retains every property of its
Gear/Port/Cord subjects. Previously it reused the workbench aspect filter, which
excluded Plan/Play provenance from Structure despite native Details retaining it.
The multi-aspect workbench keeps its existing partition. Exact IDs/endpoints keep
their existing disclosure depths; no new authority or graph store is introduced.
This is a local implementation correction for #5516/#5518, not cross-Mask proof.
The previously built ISO predates this correction.
`cargo +stable check -p patchbay-application` passed for this correction;
no test suite, image rebuild or proof journey was run.

## Patchbay card label measurement

Port labels, origin/provenance lines and Port-overflow notices now use the same
pinned-font, pixel-width preview admission already used for Gear headings. A
character-count limit could wrap wide glyphs inside a fixed-height card. Each
preview now fits its actual card text rectangle and marks clipping with an
ellipsis; complete source labels remain in Details. Geometry and exact hit
identities are unchanged. Native compositor compilation passed in 9 seconds;
no runtime proof or image rebuild was run. This advances #5629's text readability
boundary without claiming complete font coverage or shaping.

## Look restoration after graph fallback

Same-Face encounter retention now retries a still-active graph-view request after
a Look change. Enlarged text can realize that request as measured Details; a
later fitting Look restores the requested diagram and its originating diagram
page instead of retaining a stale refusal notice. Explicitly leaving graph
reading cancels the retry. Refusals still do not alter Face or create an action.
The earlier ISO predates this correction. Runtime scale/restore proof remains
deferred.
Native compositor compilation passed for the final correction (4 seconds).

## Wider continuation measurement retry

A narrow-lane InvalidExtent during field-cluster or row measurement no longer
preempts the existing wider continuation of the second authored region. Only a
strictly wider continuation can retry that failure, and each row advances at
most once. The retry remeasures indent, text width, wrapping and command cost;
other errors and a failure at full continuation width remain explicit refusals.
Native compositor compilation passed (4 seconds). No regression suite, image
rebuild or QEMU proof was run tonight; #6158 acceptance remains open.

## Pending deterministic pagination regressions

Added two CPU regression specifications in `layout/panels/continuation_tests.rs`:
a sole populated region keeps full width through continuation, and naming
help/action/current-value move together to full-width continuation when the
20-row cap prevents their cluster fitting the first page. Assertions retain
source text/control identities and check disjoint bounds. Existing small test
builders are shared. These new tests were neither compiled nor run tonight;
only whitespace validation was performed. They are pending verification, not
passing evidence or a replacement for the required source-pinned QEMU comparison.

## Five-step goal audit (live observation)

GitHub currently reports #6055 merged, and #5990/#5607/#5992 closed. The distinct
#6158 continuation regression remains open. Its local implementation and pending
regression specifications do not establish its required normal-product before/
after evidence. Remaining native Look/fallback acceptance (#5628/#5629/#5608/
#6190) and Patchbay navigation/cross-Mask/QEMU acceptance (#5516/#5518/#5519)
remain open and intentionally deferred tonight.

The #5358 parent also requires retirement of downstream PatchbayCanvas adapter
paths. Source inspection distinguishes current canonical native graph display
(`front_door/projection.rs` appends exact graph subjects) from native resident
application text inspection (`native_workset/application_delivery.rs` explicitly
selects TextInspection under its 3 KiB view admission). Browser Tour still exports
a lowered ApplicationView with PatchbayCanvas, and resident Patchbay's default
InlineGraph producer still serializes the canvas payload. Shared native graph
navigation alone does not retire these consumers. A migration must preserve their
actual actions, revision checks and admitted view bounds; deleting the vocabulary
or increasing the native view budget is not a substitute. This remains an
implementation obligation to inspect before claiming the entire Patchbay path
complete, independently of tonight's deferred proof.

## Shared graph-to-Face migration seam

Extracted the existing native graph projection unchanged into the no_std
Patchbay application crate's `graph_face` module. Native retains a narrow
re-export at its old path. This removes Host ownership of the renderer-neutral
subject/property/relationship projection and gives remaining application/canvas
consumers a concrete shared path. It adds no dependency, graph store, rendering
policy, action, new limit or implicit Face admission; callers still validate and
admit their completed Face. Implementation text matches the prior native source
after documentation/visibility normalization. Native-compositor compilation
passed (5 seconds). Full extraction validation (workspace tests/Clippy) remains
deferred under tonight's instruction; no PR or CI run was created. The canvas
consumers themselves are not yet migrated by this extraction.

## Port/Cord fidelity before consumer migration

The shared graph Face now retains exact Port IDs, Port and Cord temporal
contracts using `PortTemporal::as_str`, and offered Port abnormal-kind identities.
The existing native projection omitted these facts while the old canvas retained
temporal data. This closes that specific semantic loss before migrating another
consumer; it does not claim the entire adapter migration. New properties correctly
change the resulting Face identity and remain subject to ordinary complete-Face
admission and current-Show checks. No old Face/image is relabelled as this source.
Application event/wire bounds and action authority are unchanged.
Native-compositor compilation passed (5 seconds). No tests/proof/image rebuild
or CI jobs were run for this delta.

## Focused continuation regressions executed

Both new continuation regressions now pass: sole-region full-width continuation
and naming help/action/current-value grouping at the row cap. Exactly two tests
ran, 1,255 were filtered out; no workspace suite or platform journey ran. The
first compile attempt failed before test execution because the earlier diagram
test extraction retained a stale relative fixture path. Corrected that one path;
the same selected run then compiled in 14 seconds and passed both tests. Local
logs `native-local-continuation-regressions.log` and
`native-local-continuation-regressions-fixed.log` retain those distinct outcomes.
This supersedes only the pending-execution status of these two specifications,
not #6158's source/image-bound normal-product acceptance or other unrun tests.

## Focused graph reading checks

Two new deterministic native graph-reading checks pass against the retained
Patchbay Face specimen: repeated exact Gear/Port/Cord selection beyond 64
operations retains every selected subject property at Exact depth, resolves its
Details page and assembles its frame; stale-revision and absent-subject requests
leave the current cursor/page/Face unchanged. Exactly two tests passed, 1,257
filtered out; compile took 14 seconds and execution 0.14 seconds. This is semantic
reading/frame evidence, not physical keyboard/pointer enactment, browser/linear
comparison, QEMU or CI. The browser workbench's richer Plan/Play augmentation
remains intact; replacing it wholesale with the smaller projection would lose
facts, so consumer migration remains open rather than being claimed here.

## Graph keyboard/pointer routing

Added two focused deterministic input-path regressions: Tab traverses every
exact graph subject, Shift+Tab reverses, Enter on reading focus submits no action,
and Escape restores the originating diagram page. Pointer coverage exercises
emitted Gear/Port rectangles and uncovered Cord segments, verifies selected
identities against the same Face, and refuses a Show transitioned to Replaced
without moving selection. The fixture acknowledgement remains a fixture, not an
observed Host effect. All four graph-reading tests pass (1,257 filtered out),
compile 14 seconds / execution 0.22 seconds. The initial compile caught an invalid
test-only lifecycle variant; corrected it to the existing legal Show transition
and retained the failed and successful local logs separately. No browser suite,
QEMU journey, whole-workspace suite or CI job ran. #5516/#5519 product acceptance
and remaining adapter migration remain open.

## First browser consumer of shared Port meaning

The browser workbench's canonical Face producer now delegates both composition
Front Ports and ordinary Gear Ports to the shared Patchbay Port projector used
by native. Its exact existing human labels, value-contract properties, semantic
subject reuse and separate Plan/Play augmentation remain intact. Direction,
value kind, temporal contract, Port ID and abnormal-kind facts now come from one
projector. Temporal wording uses the existing canonical `PortTemporal::as_str`
vocabulary instead of Debug output; the workbench Face identity therefore changes
truthfully. No transport/action protocol or canvas schema was changed.

The shared Port input borrows only the three collections it changes, without
unused text/disclosure scratch state. This retires duplicated Port projection
in a real browser consumer; broader legacy canvas retirement remains unfinished.
Final frozen-source compilation of native compositor and workbench passed
(5 seconds). An intermediate API-edit compilation failed against an older
compiled interface and is excluded; its log is retained separately. No tests,
image rebuild, browser/QEMU journey or CI jobs ran for this delta. Existing
cross-Mask and stable acceptance remain open.

## Shared Cord consumer migration

Native graph composition and browser workbench now share exact Cord subject,
ordered source/sink identities, value kind, temporal contract and Connects
projection. Browser full labels, collapsed-boundary properties and Plan/Play
augmentation stay in its existing layer. Removed the browser's redundant
semantic-subject lookup vector. Port/Cord projection borrows the same three
collections through `GraphFacts`; no wire or transport format changed.

Endpoint relationships are emitted only for existing exact Port subjects.
Missing endpoint identities remain explicit source/sink properties, without a
fabricated subject or dangling Face relationship; Masks can retain honest
omission and complete Details. This aligns the shared projector with the former
browser behavior while preserving ordered endpoint truth. Native and workbench
compilation passed (5 seconds). No tests, image rebuild, QEMU/browser journey or
CI job ran for this delta; missing-endpoint runtime/conformance and full canvas
retirement remain open.

## Shared Gear and checked Back projection

Native and browser now share ordinary Gear and Plot-backed Gear base projection:
exact semantic/kind/checked-Back identities, source Plot/path provenance and
recursive containment derived from the retained checked graph. Existing browser
labels, reviewed-Back/realization facts, icon tokens, authored controls and
Plan/Play augmentation remain in their original layer. Native gains the source
provenance and recursive containment previously present only in the workbench;
its resulting Face identity changes truthfully. Geometry supplies none of it.

Both native-compositor and workbench compile on the final source without the
removed obsolete import warning. No tests, image rebuild, QEMU/browser journey
or CI jobs ran for this delta. This migrates the ordinary/Plot-backed Gear
consumers; it is not complete cross-Mask acceptance or retirement of all legacy
canvas transport. Native global-Face bounds and all renderer limits remain in
force, with no expanded admission claim from compilation.

## Typed contracts and root Plot Ports

The shared Port projector now retains optional checked value contracts for
ordinary, composition Front and root Plot Ports. Native previously omitted those
facts; browser keeps its existing contracts through the same implementation.
Browser canonical graph projection now also includes the graph's root Front
inputs/outputs before Cord projection, so exact boundary endpoints are retained
and can receive the existing Cord relationships. No endpoint is inferred from
geometry, no typed input/action authority changes, and no contract is reduced
to label parsing. New resulting Face identities remain ordinary content changes.

Final native-compositor and workbench compilation passed (5 seconds). No tests,
image rebuild, browser/QEMU journey or CI jobs ran. Root-boundary rendering,
contract conformance and complete legacy canvas retirement remain open; the
unchanged complete-Face and renderer bounds still govern admission/fallback.

## Browser root-boundary handle orientation

Browser scene Port records now explicitly mark containment by a Plot as a root
boundary. Its handle faces the graph interior: an externally receiving Plot
input supplies an internal Cord, while a Plot output receives an internal Cord.
Ordinary Gear/Back Port orientation is unchanged. Semantic direction, exact Port
IDs, Cord endpoint order and source-owned action validation remain authoritative;
only ReactFlow handle type/side and its boundary accessibility annotation change.
No endpoint or edit authority is inferred from geometry.

Both changed JS modules pass Node syntax checking. No browser enactment, image
rebuild, platform proof or CI job ran. Runtime boundary rendering/interaction
acceptance remains open; this is implementation plus syntax evidence only.

## Honest browser Cord omission notice

The browser scene now retains a derived count of Cords in the current semantic
projection. The diagram compares that count with its actual non-hidden edge
state and presents a status notice when geometry omits Cords, including missing
endpoints/closed Backs or collapsed workspace frames. It explicitly says
"projected Cords", without claiming the current lens is the whole Face. Full
Face identities/properties remain unchanged and inspectable; no graph store,
connection, action or renderer bound is introduced by the count.

The notice uses existing Look tokens, wraps within the canvas and does not
intercept pointer input. Both changed JS modules pass syntax checking. No browser
enactment, image rebuild, QEMU proof or CI job ran; actual visual/accessibility
and omission-case acceptance remain open.

## Explicit resident Fore migration prerequisite

Filed #6201 under #5358/#5356 for the remaining production resident Patchbay
adapter retirement. Actual Source is `plots/patchbay/main.conduit`:
`input/application-event` → `application/retained` → `face/application-view`.
Browser plot_runner returns the retained application's encoded view; native's
separate 3 KiB TextInspection route coexists with canonical graph projection.
Removing the canvas codec alone would break a supported graph path. #6201 owns
its typed Face/Show/Interaction migration, real consumer retirement, preserved
operations and exact finite admission; it does not waive parent acceptance or
claim completion from the shared graph projection. No CI/proof was launched.

## Focused Look fallback/restoration checks

Added two deterministic graph encounter regressions. An exact selected Cord and
originating diagram page survive same-Face transfer into 200% measured-text
fallback; a fitting 100% Look restores the requested diagram and exact cursor,
clears the refusal notice and assembles its frame. Explicitly leaving graph
reading cancels that restoration request and retains Details. Face identity and
content are unchanged by either operation.

All six graph-reading tests pass (1,257 filtered out), compile 30 seconds /
execution 0.46 seconds. This includes the earlier semantic, keyboard/pointer and
stale-Show cases at this source. It is CPU encounter/input/frame evidence, not
an actual owner Show refresh, browser comparison or QEMU/CI acceptance. No broader
suite, image rebuild, platform journey or CI job ran.

## Resident initial observation preparation

Added `PatchbayApplicationPort::observe_fragment` as a read-only preparation
operation over the retained source model. It needs no event or fabricated
FaceInteraction. It records current inventory/selection and observed Plan/state
facts, reuses shared exact graph projection, and references rather than
redeclares the outer Face's Body/resident Plot subjects. Caller-supplied
contribution basis is provenance, not authority; final composition/Face byte
bounds and storage admission remain the owner's responsibility. No actions are
advertised before an exact return route exists.

This is a #6201 preparation seam, not a switched production Fore or a completed
startup scheduler/Show feedback loop. Existing application paths remain intact.
Native-compositor/workbench compilation passed after correcting a BodyId import
from core to its existing body crate (failed initial log retained separately).
No tests, image rebuild, platform journey or CI job ran. Actual startup, typed
owner effects and migration acceptance remain open.

## Source-owned resident action projection

Added `face_fragment` over the initial read-only observation, following Todo's
owner-admitted-return-route projection pattern. Plot selection, explicit-subject
Edit and the current topology's next Mask request have exact source-owned action
contracts. Without an owner-admitted route they retain a typed unavailable
reason. The flag is projection input from the owner, not authority from a Mask.
Graph reading/inspection is navigation, with no new domain effect action.

Edit carries an explicit bounded UTF-8 semantic-subject argument so its meaning
does not depend on renderer cursor state. Future owner command admission must
check exact graph membership before effects, alongside current Face/Show/basis;
this projection alone neither dispatches nor authorizes mutation. The legacy
production route is unchanged. Native-compositor/workbench compilation passed
(5 seconds). No tests, image rebuild, platform journey or CI job ran. #6201's
actual typed command handler, Show feedback and production Fore remain open.

## Canonical resident command admission

Added pre-effect typed command admission for #6201. The owner must supply its
retained current Face, genuine acknowledged Show and actual Playing contribution
basis; submitted Mask evidence is not an owner context. Admission checks exact
Body/context/contribution provenance, application revision and Body Plan, normal
FaceInteraction validation and equality with the source's current action contract.
Edit additionally validates the explicit semantic subject against the selected
exact graph. Accepted requests use typed Plot/Plan/expanded-Plot identities.

Prepared commands retain owner basis, Face/Show/interaction identity, sequence and
state revision for rechecking at execution. They neither grant authority nor
mutate/dispatch effects. No ApplicationEvent coercion or retry was introduced.
Native-compositor/workbench compilation passed (5 seconds). No tests, image
rebuild, platform journey or CI job ran. Execution-boundary rechecking, actual
owner route/Show feedback and production Fore migration remain open; these
preparation APIs are not production acceptance.

## Resident command execution boundary

Added execution-boundary checks against the owner's current Body/contribution,
Face/Show identities, original validated interaction, model revision/Body Plan
and Mask mode. Prepared commands retain bounded interaction evidence; replacement
or stale owner state refuses before mutation. Canonical and legacy paths now
use one typed retained-state update implementation. Edit/Mask operations return
existing owner requests; this module performs no Host effect or retry.

All fallible domain/revision checks precede state writes. Legacy inspection no
longer changes selection before discovering revision exhaustion. Mask requests
now advance the application revision, making the same old event stale rather
than permitting repeat admission. Three focused checks pass: atomic exhaustion
across selection/Edit/inspection, exact Body-Plan Mask request plus stale replay,
and existing Edit/stale-event behavior. Native/workbench compilation also passes.
No wider suite, image rebuild, platform journey or CI job ran. Actual canonical
Face/Show production transport, ingress reservations and feedback remain #6201;
these API/CPU checks are not installed-owner or stable acceptance.

## Focused canonical resident command fixtures

Three focused canonical API tests pass: Plot selection followed by explicit Edit
returns the exact existing owner request; replayed prepared commands and replaced
Shows refuse; unknown graph subjects and forged action contracts refuse without
changing source observation. Fixtures compose outer-owned Body/Plot declarations
with the real new contribution and reuse the existing shared planned-Mask helper.
The added planner dependency is dev-only, with its required lockfile entry; no
runtime dependency/framework was introduced.

Compilation took 2 seconds, execution 0.96 seconds. Fixture acknowledgements are
explicitly not actual Host effects. No wider suite, image rebuild, browser/QEMU
journey or CI job ran. #6201's real producer/transport/Show feedback and its
finite ingress/storage admission remain open.

## Existing browser application instance seam

Extracted retained-application Host Call completion from the frozen 1,000-line
engine transformation dispatcher into `engine_application.rs`. The original
completion block is unchanged after whitespace normalization, at the same
position in dispatch. It uses the existing prepared instance in
`TourScheduler.applications`; no application state, scheduler, input port,
operation identity, storage limit or effect behavior was added.

Browser runtime compilation with its actual default/Crèche/plot-runner features
passes. Full extraction workspace tests/Clippy remain deferred under tonight's
instruction; no platform journey, image rebuild or CI job ran. The existing
Thermostat Body Face/Show publication path supplies an integration precedent,
but Patchbay's typed ingress still requires separate explicit admission. This
extraction prepares a responsible access seam; it is not production Face startup
or a side channel for injecting canonical commands into the old event Fore.

## Browser-owned canonical observation connection

Added the internal `conduit_browser_patchbay_observation` ABI over the current
browser-owned Body Play. Preparation binds optional observation context to an
exact foreground Patchbay placement and actual Body evidence/Wake/Plan. At
observation, the engine reads its existing prepared Patchbay instance, refuses
ambiguous instances, and composes the current source contribution with BodyFace.
No domain application clone or second scheduler/store is created. No event or
empty FaceInteraction is needed to obtain initial meaning.

The response explicitly carries no MaskShow, reports interaction not admitted,
and retains unavailable Patchbay action contracts until a typed route exists.
It does not realize a graphical Mask, bypass the old event Fore, dispatch an
effect or replace the working legacy interface. Missing evidence/ambiguous
foreground placement disables this optional context rather than rejecting old
supported startup. Retiring Plays refuse observation.

Final browser runtime compilation passes (3 seconds). No tests, image rebuild,
WASM/browser/QEMU enactment or CI job ran. Actual observation enactment, current
Show feedback, exact typed ingress/storage reservation and final consumer switch
remain #6201 acceptance work.

## Browser observation CPU checks

Two focused Body-session checks pass: observation yields a validated current
Body Face with unavailable actions and no Show, repeated reads preserve pending
effects and active Play identity, and missing optional biography evidence leaves
legacy Patchbay startup working while refusing optional observation. The first
run failed before observation because the general fixture omitted the reviewed
inventory Source; binding the actual Patchbay Source corrected the fixture.
Both logs remain in the task work directory. These checks do not enact WASM,
DOM, a Host acknowledgement or typed production ingress. No broad suite or CI
job was launched.

## Bind canonical observation to the actual resident node

Removed the scheduler-wide Patchbay type search. Preparation now matches the
foreground resident's exact Plot Plan, fragment and placement to its composed
local kernel node, and observations address only that instance. Multiple
Patchbay residents no longer make an otherwise exact foreground observation
ambiguous; an absent/wrong bound node refuses without choosing a substitute.
Externally managed or ambiguous owners retain no optional observation context.

The two new CPU checks cover a two-Patchbay Body and refusal of a missing bound
node. Their initial multi-resident fixture used a concatenated Source document,
which changes the original document identities and was correctly refused by the
reviewed inventory. The corrected fixture uses the existing reviewed bundle
format, preserving each Source identity. No production inventory bypass was
introduced. Typed transport/Show publication and consumer replacement remain
unfinished #6201 work; no platform or CI proof ran.

Final exact-owner run: 4 passed, 0 failed, 447 filtered out (CPU only).

## Connect canonical observation to the browser caller

The bounded runtime bridge now reads `conduit_browser_patchbay_observation`
without submitting input, preserves runtime refusal evidence, and rejects a
response that claims a Show or admitted interaction. The existing Body Host
adapter applies its current-lifetime guard. `BrowserPlay.patchbayObservation()`
additionally checks the actual Body, Plan and active Play identities before
returning the canonical Face. Type declarations and SDK documentation retain
the read-only/development boundary. No application state, queue, renderer or
owner effect was added, and the legacy interaction route remains working.

Three JavaScript syntax checks pass. Two focused bridge CPU checks pass,
including memory growth during observation, exact output forwarding and refusal
of malformed schemas/claimed interaction authority. This is a mock ABI fixture,
not WASM/browser/platform enactment. No broad tests, CI or image rebuild ran.

## Canonical wire-to-owner callback

Added `PatchbayApplicationPort::apply_face_interaction`: the existing bounded
canonical codec feeds owner admission and the shared retained-state execution
path. Current Body/basis/Face/Show arrive independently from owner state, never
inside the submitted frame. No sentinel conversion, new codec, hidden queue,
transport grant or direct Host effect was added. Actual finite transport
reservation and production invocation remain unfinished #6201 work.

Two selected CPU checks pass (three other tests filtered out): valid canonical
bytes apply once and replay refuses; empty/sentinel/JSON/trailing/oversized
frames, an unacknowledged Show and a foreign Body refuse without model changes.
The first refusal fixture tried the forbidden Available-to-Failed transition;
its negative log is retained and the fixture now constructs a genuine Prepared
Show. The production state machine was not relaxed. No broad suite, CI, image
rebuild or platform proof ran.

## Portable canonical Fore and browser Back preparation

Added the domain-owned `patchbay/accept-interaction` Kind, a separate canonical
closing-Fore Source, and a browser Back offer/preparation primitive. The sink
uses the existing kernel Host Call verb with one pending request, no initial
sentinel and no output. Its exact contract retains the canonical 8 KiB bound
and one queue item. Preparation rejects old event/open-flow types and widened
limits. Ordinary browser capability advertisement, factory registration and the
legacy resident entry remain unchanged until actual owner/transport admission.

Browser compilation passes after moving the module into its owning installed-
Back namespace (initial private-module access error retained in its log). A
focused preparation test passes; it checks authored Source expansion, canonical
Mask-output correspondence, valid preparation and legacy/open/widened refusal.
No actual Host Call was dispatched, owner route installed, storage reserved by
this new primitive, Show acknowledged, platform proof or CI job run.

## Plan the canonical ingress through the ordinary planner

Added `PlannedPatchbayIngress`: checks/expands the new Source, selects only from
an explicitly supplied Host advertisement, validates the exact Back and lowers
the sealed Plan. The explicit input boundary is one closing canonical Flow,
one slot and 8,192 bytes; no external Line or ambient legacy owner is selected.
The result exposes immutable Plan/lowering/input facts for actual installation.
It is not a running child Play or a parent resource reservation.

Browser compilation passes. One focused CPU check passes (452 filtered out):
sealed Plan verifies and retains supplied Host/Boot, lowering charges exactly
one slot/8,192 payload bytes/one node/one Host Call, and absent or altered offers
refuse. A local formatting syntax correction added parentheses around an enum
struct literal before compilation. No CI, platform proof, image rebuild or
broad test suite ran. Kernel storage installation and current owner/Show binding
remain #6201 implementation work.

## Install canonical ingress kernel storage and tables

Added a prepared one-frame transport using the existing fixed kernel, exact
lowered node/cord/routes/Host Call bindings, one 8 KiB value slot and fixed Sign
and pending-call storage. It closes input after admission and exposes bytes only
from the emitted Host Call. No retained application model or hidden command
queue is duplicated. Completion admits no output; consumed transport cannot
retry. Parent Body storage/Play admission and current owner/Show binding remain
unwired, and no production actions are enabled by this preparation primitive.

Browser compilation passes. One focused CPU fixture passes across Completed,
Failed, Denied and Cancelled outcomes (453 other tests filtered out): maximum
8 KiB bytes arrive unchanged, empty/oversized inputs refuse, resubmission refuses,
terminal state is retained and value allocation capacities do not grow. The
first run correctly surfaced kernel BackFailed, exposing missing terminal-state
recording in the wrapper; that was corrected while retaining the failure. Its
negative log remains. The generic sink was extracted from the 656-line Back
file to add explicit cancellation propagation without growing that file.
Full extraction workspace tests/Clippy are deferred under tonight's instruction;
only this directly relevant fixture ran. No Host owner effect, current Body
installation, platform proof, image rebuild or CI job was claimed or launched.

## Connect emitted canonical Host Call to retained owner callback

Added `dispatch_to_owner` and a must-use receipt that retains the exact ingress
Plan identity, typed owner result/request and separate transport completion.
It reads actual emitted kernel-call bytes, uses independent current owner
evidence, invokes the shared canonical callback, and completes the planned call.
Wrong owner/contribution is Denied; revision exhaustion retains an identity
capacity failure; other domain refusals remain failed input with their full
typed refusal in the receipt. An Edit/Mask request is not dropped merely because
later kernel completion is uncertain. No effect is dispatched or implicit retry
introduced. Current Body actor/Show/storage installation remains unfinished.

Browser compilation and two focused CPU kernel/model/Show fixture checks pass
(454 filtered out). A real checked Source graph reaches typed EditCurrent with
one owner revision increment; consumed transport and stale replay refuse;
malformed bytes and replaced Shows fail before owner state changes. An initial
fixture assumed revision zero, while `open` starts at one; the corrected test
checks the exact increment and retains the first failure log. Fixture Show
acknowledgements are not page Host evidence. No broad tests/CI/platform proof or
image rebuild ran.

## Revoke old Show transport before owner dispatch

Added explicit ingress retirement for Show replacement and Body retirement.
Revocation is set before kernel cancellation, and blocks admission, pending-byte
access and late completion even if cancellation fails. Successful retirement is
idempotent. It does not pretend to cancel an Edit/Mask request already returned
to the owner. The eventual product lifetime caller remains unwired.

One focused CPU check passes (456 filtered out): pending and ready transports
retire, repeated retirement is harmless, late completion/resubmission/owner
dispatch refuse, owner projection remains unchanged, and storage capacities stay
fixed. No broad suite, CI, platform proof or image rebuild ran.

Live goal-state refresh: #6055 MERGED; #5990, #5607 and #5992 CLOSED; #6158 and
#6201 OPEN. These issue states do not supply the still-deferred native/product
proof or finish Patchbay production installation. The full goal remains active.

## Retain current acknowledged owner turn and its result

Added `PatchbayOwnerTurn` over moved owner Face/available Show and unused
prepared ingress, without cloning an application model. Binding validates exact
Face/Show/Body/contribution/context correspondence using the existing domain
predicate. It stores the full typed dispatch receipt before returning; retiring
or transferring that receipt cannot discard a request, overwrite it or reopen
the consumed turn. Current retained owner validation still runs at dispatch.

Browser compilation and two focused CPU fixtures pass (457 filtered out): exact
Edit request/revision increment survives retirement and single ownership
transfer; wrong contribution scope/unavailable Show refuses; retirement before
submission leaves model unchanged. No platform acknowledgement was established,
production route installed, broad suite/CI run, or image rebuilt. Current Body
storage/Play admission and actual Show publication remain unfinished #6201.

## Compose Source, ordinary Mask, acknowledged Show and owner ingress

Added `PatchbayPublication`: owns a prepared Mask/ingress, accepts exact complete
Host acknowledgement, binds the retained owner turn, preflights current source
action admission, consumes actual Mask-emitted typed bytes through the ingress
kernel and retains the owner outcome. Retirement only revokes return authority;
it does not invent physical display closure or cancellation of returned effects.

Browser compilation and one focused composed CPU check pass (459 filtered out).
The fixture prepares the real resident Source/application, records the normal
Body Plan-ready/Play-started lifecycle, selects a Plot through an acknowledged
Mask/ingress turn, then publishes fresh source meaning and returns exact Edit
through a second turn. Revisions advance once per action. Before-ack, stale ack,
forged Show identity and old-turn reuse refuse. Host acknowledgement is synthetic
fixture input, not DOM/browser enactment or a currently installed product route.

Earlier fixture attempts were correctly refused: first no current Play had been
recorded (`PlayNotCurrent`), then Edit was sought before source-owned Plot
selection. The final fixture follows both real prerequisites; no production
lifecycle/action check was weakened. Negative logs remain beside the passing
log. No broad tests, CI, platform proof or image rebuild ran. Production Body
storage/Play selection, effect publication, exact resident installation and
owner-request handling remain unfinished #6201 work.

## Stop discarding production browser Patchbay owner requests

Inspection found the actual retained adapter used `.map(|output| output.view)`,
silently losing returned Edit/Mask owner requests. Added `PreparedPatchbay`,
which wraps the same port and one pending request plus application revision.
It moves the request into that finite slot before returning the view, preserves
it through empty observations, and refuses further nonempty input before model
mutation. Exact-node current observation now exposes `pending_owner_request`;
SDK declarations and development documentation describe its unfinished handler.
No application clone, new Host capability or automatic effect dispatch was added.

Browser compilation and one focused CPU adapter check pass (460 filtered out).
It uses the current Body session's actual prepared instance: canonical inventory
Source, legacy Plot selection/Edit, retained exact expanded Plot/subject/revision
in observation, refusal of a valid overwrite action, stable empty/repeated reads.
The fixture calls the production adapter directly; it is not Host-effect or DOM
proof. Disposition/effect handling and durable cross-Play delivery remain absent;
this checkpoint does not claim Edit was performed. No broad tests, CI, platform
proof or image rebuild ran.

## Resolve retained owner requests without pretending to execute effects

Added current-Play request disposition from bounded ABI through Host adapter and
SDK. Exact Body/Plan/Play/owner checked Plot/application revision and a nonblank,
256-byte Host evidence identity are required. One latest typed Host report
retains its original request; Uncertain keeps pending pressure, while Completed,
Rejected and Cancelled reports release it. This records what the Host reports,
not proof that an Edit/Mask effect ran. No Source values or application revision
change merely from recording the report. Replay/foreign/stale reports refuse.

Corrected the prior pending-input guard: returning an adapter error could fail
the runtime state gear. Pending pressure now returns a checked busy legacy view
with actions removed and a structured refusal, retaining Source text/values and
allowing normal action presentation to resume after disposition. The Host uses
one existing empty observation through the original bounded event/state/view
route to refresh, preflighting queue pressure before recording the report.
This is observation only, not an effect retry or a canonical-command side channel.

Final focused Rust adapter/disposition check passes (460 filtered out): exact
request retention, busy/refusal behavior, wrong Play/revision/evidence refusal,
Uncertain retention, Cancelled resolution with original request retained,
replay refusal, unchanged domain revision and later action progress. Four
Patchbay bridge CPU checks pass, covering exact forwarding, byte bounds, memory
growth and input retirement on refusal. Three JS syntax checks pass. The Host
refresh branch and SDK operation were not enacted in a browser. No broad tests,
CI, platform proof or image rebuild ran. Actual effect handling, canonical
production installation and remaining native acceptance stay open.

## Connect Handbook Edit to the existing checked Source editor

Added an optional owner request handler on Body wake and Host presentation. It
runs only for a planned retained-Patchbay owner with matching current identities,
remembers one request key to avoid automatic retries, and reports its outcome
through the existing scoped disposition path. Source editing remains an owner
application operation, not authority granted by renderer labels or graph geometry.

Handbook now resolves exact Source/Plot/subject/containment facts and opens an
admitted lesson in its existing editor, preserving drafts; the existing checked
Try operation remains the commit. It rejects ambiguous/foreign/unresolved nested
or unadmitted targets and unsupported request families explicitly. No span is
guessed. The new helper's import and 4 KiB package resource are declared in the
source template; no packaged artifact or ZIP was rebuilt.

Also fixed observation lifetime for the normal Handbook's one Play: if another
resident starts foreground, its sole Patchbay still binds the exact owner node.
A focused Rust check passes (461 filtered out). Its initial fixture used an
array position despite Body Plan identity ordering; corrected to use planned
kinds, negative log retained. Two Source-resolution CPU checks pass (including
foreign/ambiguous/nested/cyclic refusal), and one existing Host application-event
retirement baseline passes. Five JS syntax checks pass. Full callback/DOM editor
opening, browser packaging and real user interaction remain unenacted; no broad
suite, CI, platform proof or image rebuild ran. Canonical product installation,
Mask-change handling and remaining native proof remain unfinished.

## Keep requested editor opening within its effect lifetime

Review found that the Handbook Edit handler awaited preference storage before
focusing the editor/reporting success. A slow write could resume those UI changes
after the Play retired or the user selected another lesson. The Host now passes
the actual effect AbortSignal to owner handlers; Handbook checks cancellation
before UI mutation and opens requested Source synchronously. Opening a document
preserves drafts but does not overwrite the user's saved lesson preference.
Explicit lesson selection retains the existing awaited preference write.

Two changed JavaScript modules pass syntax checks. No tests/builds/platform
journey/CI/image rebuild ran for this focused lifecycle correction. Actual
browser callback enactment, canonical production installation, Mask changes and
remaining native acceptance are still unfinished.

## Follow exact checked Back provenance for nested Source editing

Extended the existing editor target resolver to follow an internal subject to
its nearest exact containing checked Back declaration in the checked inventory.
Composition invocation edits still select the outer Source. Source-plot identity
must agree, and recursive-parent facts must match unique containment. Captions
are ignored. Missing/ambiguous Back declarations, mismatched facts and nested
Cords without direct declaration provenance refuse rather than guessing a file.

Four focused resolver CPU checks pass, including exact nested declaration and
outer invocation routing plus unknown/ambiguous/inconsistent refusal. Syntax
check passes. The helper is 4,258 bytes; its source package resource admission
was explicitly raised from 4 KiB to 8 KiB. No package/ZIP, build, broad suite,
CI or platform journey ran. Actual browser enactment and the full canonical
production/native acceptance remain unfinished.

## Track and fence missing Cord declaration provenance

Created #6202 under #6201 for exact authored/generated Cord provenance through
expansion, PatchbayGraph and shared Face. Inspection found Gear provenance but
only endpoint/type/track/temporal fields for flattened checked connections.
The resolver now refuses unproven Cord ownership at the root as well as through
nested Backs. It cannot infer authorship from endpoint containment.

Five focused resolver CPU checks pass, including refusal without declaration
provenance and acceptance when the explicit Source fact matches the checked
owner. Syntax check passes. No build, broad tests, CI, platform journey or
artifact rebuild ran. This is a correctness fence pending #6202, not fulfillment
of Cord editing or the full interaction goal.

## Establish provenance ownership before extending Cord origin (#6202)

Extracted `ExpandedGearProvenance` and its canonical digest encoding into
`checked_syntax/expanded_provenance.rs`. The module depends on provenance data,
lexical spans and the existing hash primitive, not expansion orchestration,
planning or Plot consumers. The existing public type path remains re-exported;
fields, digest prefix, field order and length-prefixed encoding are unchanged.
The old checked-syntax module is now 494 lines; the new responsibility is 40.
No Cord origin data or changed identity algorithm was added in this checkpoint.

A direct body comparison confirmed the digest body was preserved verbatim.
Formatting and one existing selected-Back/provenance validation check pass
(472 filtered out). Initial compilation found the moved hash helper needed an
explicit import; corrected, negative log retained. Full workspace tests/Clippy
remain deferred under tonight's explicit instruction; no broad build campaign,
CI, platform journey or artifact rebuild ran. #6202 continues with actual origin
propagation after this extraction seam.

## Retain checked Cord Source extents (#6202)

Extracted checked Cord data and stage checking into responsibility modules,
keeping existing public paths. Checked Cords now retain the whole ordinary Cord
or matched-route arm span. Plain reference-only Cords therefore no longer lose
all location information at checking. The new metadata is excluded from semantic
equality and the existing canonical encoding; stage semantics are unchanged.
Updated the two workbench construction fixtures.

The plot crate compile check and one focused reference-Cord extent/equality/
canonical-encoding test pass. Full workspace tests and Clippy for the extraction
remain explicitly deferred tonight. These extents are not yet propagated through
flattened connections or projected into Patchbay: #6202 and #6201 remain open.
No CI, platform journey, artifact rebuild or closure claim accompanies this work.

## Preserve connection origins during expansion (#6202)

Extracted connection materialization from the large graph module in a separate
commit. Added a preparation-only connection inventory that retains origin beside
its exact connection while recursively merging fragments. Each checked Cord
attributes only previously unowned rows created during its expansion; it never
infers ownership from endpoint names. Selected Backs bind their lexical Source
document before merging, so nested/local child origins survive caller wiring.
Root expansion binds remaining local origins to the root Source document.

Expanded connection provenance contains the full ordered connection contract,
Source document, checked Plot, Source Plot name, instance path and whole checked
Cord/route-arm span. Cord-stage wiring, relational-operand wiring and generated
semantic-expression wiring are distinct classifications. `CordStages` describes
checked stages, not a claim that normalized Source was literally authored in that
form. Internal generated wiring is not promoted to a directly editable declaration.

The existing expanded primitive graph identity remains unchanged by connection
location metadata. The separate provenance digest now includes a versioned,
length-prefixed connection-origin section, with exact contract/track, all span
coordinates and individually encoded path segments. Validation checks both that
digest and one provenance row per exact connection, including multiplicity.
Missing origin during compiler finalization fails explicitly.

Focused local checks pass: existing selected-Back expansion test; two new tests
for a local Plot inside a selected Back, fan-out and missing/span/track mutation;
and the existing nested semantic-expression test expanded to check its generated
connection origin. No full suite, workspace Clippy, CI, platform journey or
artifact rebuild ran. Graph/Face propagation and editor use remain pending;
#6202 and #6201 remain open.

## Carry Cord origin through graph, shared Face and editor (#6202)

PatchbayGraph now retains each uniquely matched exact expanded connection origin.
It verifies the Source provenance digest/inventory through a narrow shared
`validate_source_provenance` method. Corrupt mappings are an explicit graph error;
ambiguous duplicate connection origins and boundary bindings have no proved
editable origin. Existing graph and Face admission bounds are unchanged.

The shared browser/native Face projector emits origin classification, Source
document and checked Plot identities, Source Plot name, length-prefixed instance
path and all six Source extent coordinates. A separate small module owns those
facts; Mask geometry and Look remain downstream. Unknown boundary provenance is
explicitly `unproven`, rather than inferred from endpoint containment.

The Handbook resolver now requires the Cord's own exact document/checked Plot/name
match in the admitted editor inventory, independently of its layout containment.
It refuses generated internal wiring, absent or duplicate facts, stale identities,
invalid extents and UTF-8 code-point splits. Valid byte extents become browser
UTF-16 selection ranges. The editor selects that extent only if the displayed
Source still equals the checked Source; existing drafts remain intact. The helper
is 5779 bytes, within its existing 8192-byte application declaration.

Graph and application compile checks pass. One focused graph-origin/corruption
test and seven small resolver CPU tests pass; application JavaScript syntax passes.
An initial resolver test removed the root document fact as well as the intended
Cord fact; scoped the fixture mutation to the Cord and reran successfully. No CI,
DOM journey, image/ZIP rebuild or stable acceptance is claimed. Face admission at
larger graph sizes, boundary-origin propagation, remaining #6202 acceptance and
canonical resident installation under #6201 remain open.

## Preserve root Fore Cord declarations (#6202)

Boundary binding construction now retains its current checked Cord span and origin
classification. When a root Fore connects through a child Plot, the binding is
attributed to the caller Cord that materialized it, rather than the child's
internal export. Root authoring projection combines those compiler-retained facts
with its exact root Source/checked Plot context; it does not discover ownership
from endpoint names. Absent provenance remains explicit for synthetic callers.

A separate versioned boundary provenance digest binds direction, ordered Fore/
Gear endpoints, track and origin coordinates to the root document/checked Plot.
Patchbay validates it before projecting authoring boundaries. Boundary Cord
identities now include track and length-prefixed ordered endpoints, matching the
existing internal Cord identity law. A small shared graph Source record supports
both internal and boundary origins through the same Face/editor projection.
Updated the one manually constructed numeric-test binding with explicit unknown
origin; it is not promoted to compiler-proven declaration truth.

The plot and application compile checks pass. One focused graph test passes for
both input/output boundary declarations through child expansion, including
corrupt-span refusal. Removed an unused import reported by compilation. No broad
suite, CI, platform journey or artifact rebuild ran. Face admission coverage,
remaining #6202 acceptance and production canonical resident migration under
#6201 remain unfinished. This is local implementation, not stable acceptance.

## Admit native owner-request pressure before effects (#6201)

The native application Host effect previously executed the actor before checking
whether its owner-request slot was occupied. If a second result had no request,
it could also replace the pending request with None. The existing kernel event
entrance now refuses occupied request storage before consuming its input Host
Call; the effect itself checks before invoking the actor. Patchbay target changes
and Mask-topology refreshes preflight pending storage before retained mutations.
The Journey's single-slot handoff likewise refuses a subsequent event before
encoding, removing the current view, advancing the kernel or changing revision.

The change uses existing request slots and InputPressure, with no new actor truth,
queue, automatic retry or allocation. Taking the existing request releases its
slot normally; cancellation retains its existing explicit terminal behavior.
Two focused regressions cover kernel Patchbay view/request preservation under
pressure and Journey projection/request preservation. Native editor fulfillment
and canonical resident installation remain unfinished #6201 work. No QEMU, CI,
image/ZIP rebuild or stable acceptance is claimed.

Both focused native CPU regressions pass (one kernel Patchbay case and one
Journey handoff case; 1262 other tests filtered out in each invocation). Initial
compilation exposed a native synthetic graph fixture missed by the earlier Cord
origin field change; fixed that fixture to mark its origin explicitly unproven.
The failed compile log is retained locally alongside the two passing logs.
No broad suite was run.

## Canonical dispatch shares the resident owner's pending slot (#6201)

Added a narrow installing-owner interface to the existing prepared ingress,
owner turn and Mask publication. The raw domain port remains supported for
callers that retain the returned receipt. The actual `PreparedPatchbay` wrapper
now implements the interface using its existing actor and pending request slot.
Canonical and legacy inputs therefore share delivery pressure and the existing
Host observation/disposition metadata, rather than introducing another queue.

Successful canonical Edit/Mask proposals transfer their owned request payload
into that slot without cloning strings. The dispatch receipt explicitly records
Returned, Retained-at-revision or NotRequested delivery, separately from transport
completion; retained payloads are absent from the returned payload field. This
keeps one request owner and preserves transport uncertainty as a separate fact.
Occupied storage refuses before the Mask's closing input or ingress submission
is consumed. Receipt retrieval does not release the resident pending request.

One composed CPU test passes on the final source: actual retained owner wrapper,
source-owned selection, fresh Face/Show, canonical Edit through the prepared
Mask/ingress kernels, retained request identities/revision, and refusal of another
publication without consuming its closing Mask input. Host acknowledgements are
fixtures, not DOM enactment. No broad suite, CI, QEMU or artifact rebuild ran.
Automatic installation/charging of both Plays, production presentation, legacy
adapter retirement and native editor fulfillment remain unfinished #6201 work.

## Prepare a Mask inside the captured owner Body Plan (#6201)

Extracted finite Mask preparation/evidence assembly from the >500-line runtime
module in an extraction-only commit. The runtime facade is now 340 lines and the
preparation responsibility is 257. Existing recipe callers retain their behavior.
Full workspace extraction checks remain deferred under tonight's instruction.

Added `BrowserMaskRuntime::prepare_in_body_plan`: validate the captured Plan/Wake
and Body/Face correspondence, require the exact installed Mask chains already in
that Plan, and prepare their existing finite kernel without adding topology or
sealing a replacement Plan. The existing prototype recipe is not silently used
as production owner admission. Original Body Plan data and identity survive Show
preparation and acknowledgement unchanged.

PatchbayPublication now requires its Mask's Body Plan identity to equal the
owner contribution basis and requires exact Face/Body correspondence. The
composed test seals Mask topology before owner preparation and uses the new
captured-plan entrance; it no longer attaches a shadow resealed Body Plan.
This establishes planning identity correspondence, not aggregate storage/resource
reservation or automatic production installation.

Two focused CPU cases pass on final source: captured-plan preparation (missing
routes and foreign Body refuse; exact Plan survives acknowledgement), and the
existing composed Patchbay owner/Mask/typed ingress/request-retention case using
one captured owner plan. Initial test-module lookup failed because the parent
uses an explicit path; corrected its child path and retained the failed log.
Acknowledgements remain fixtures. No broad suite, CI, DOM/QEMU journey or artifact
rebuild ran. Parent storage admission, live publication, adapter retirement and
native editor fulfillment remain unfinished #6201 work.

## Remove excess Mask value backing before aggregate admission (#6201)

Inspection found the Mask kernel allocated twelve uniform 512 KiB value slots
(6 MiB payload backing), although its closing lifecycle creates only three
payloads: supplied Face, acknowledged Show and emitted interaction. Queues and
Host Calls retain references to those values; their table capacities do not
create additional distinct payloads. Preparation now reserves all three at once
(1.5 MiB payload backing), saving 4.5 MiB per prepared Mask without changing the
512 KiB per-value bound, generic Host pool, kernel routes or Source contracts.
The live-byte quota covers those three full-size frames.

Repeated acknowledgement previously allocated a Show before discovering that
the Host request was already consumed. An acknowledgement-attempt fence now
precedes allocation/Host completion, so partial or uncertain completion cannot
leak frames through retry. Mismatched evidence still refuses before that fence.
Interaction admission likewise checks for its live request before allocating a
frame, including after partial completion. Availability still requires the
existing complete acknowledged Show path; the fence does not manufacture proof.

Mask and ingress expose their actual prepared value-store backing, including
slot metadata, for parent accounting. These measurements explicitly exclude
Sign/kernel tables and other Host allocations; they are not a total-memory
receipt or aggregate reservation. Aggregate parent admission remains unfinished.

Two focused CPU cases pass on final source: captured-plan preparation checks the
smaller backing and eight duplicate acknowledgements with unchanged live value
count; the composed owner/Mask/typed-ingress selection/Edit/request-retention path
completes with three slots. No broad suite, CI, DOM/QEMU or artifact rebuild ran.
Automatic installation, complete resource admission and native Edit remain open.

## Parent-scoped publication value/scratch leases (#6201)

Extracted the existing parent storage witness in a separate refactor commit;
its fixed kernel storage alias now lives beside that witness. The engine facade
is 498 lines. The semantic change creates one non-clonable storage-budget owner
from the actual prepared scheduler's measured value backing and admitted Data
scratch, with the existing 8 MiB pool limit unchanged. Plan and active Play scope
are retained together. This is allocation accounting, not route authority.

Publication construction now requires that parent's budget and charges the actual
prepared Mask plus ingress value-store backing. A fixed two-slot ledger permits
current/replacement overlap, refuses foreign scopes, over-budget sums and a third
live publication, and reserves no growing collection. Leases share that ledger
without cloning a budget or allocating at admission. Retirement does not release
accounting while kernel buffers remain; lease drop follows actual buffer drop.
Parent cancellation revokes interaction before kernel cancellation receipts,
and parent budget destruction also revokes outstanding leases. Readable receipts
remain available; this does not claim erased platform pixels.

The composed case now obtains the actor and budget from actual Body preparation
and its scheduler, instead of separately preparing a raw application port. It
exercises source selection/Edit/request retention, two publications, buffer
release before replacement and real parent scheduler cancellation. A focused
ledger case covers wrong Plan/Play, overlap bounds, failed admission without a
leaked slot, release and revocation. Both CPU cases pass; no broad suite, CI,
DOM/QEMU or artifact rebuild ran. Extraction workspace checks remain deferred.

This accounts for prepared companion value buffers against the parent's existing
value/scratch pool before publication assembly/acknowledgement. It does not yet
stage automatic publication construction before Body start, count Sign/kernel/
Face/JSON/other Host memory, publish actual DOM effects or retire the legacy
adapter. Those complete-admission/installation requirements and native Edit
fulfillment remain open under #6201.

## Automatic captured-route preparation and resident commands (#6201)

Body preparation now checks the exact resident's captured Mask topology. For a
Body-composed Patchbay selector, it binds the actual scheduler-owned actor/node,
installs the canonical Back in that scoped preparation context, prepares its
checked ingress and Mask, and acquires the parent's value/scratch lease before
polling the Body's first platform effect. The generic browser advertisement is
unchanged. Missing selected topology preserves existing legacy callers; an
incompatible explicitly selected route refuses rather than being resealed or
silently substituted. No application clone or legacy-event conversion is used.

The session retains that prepared publication. Source observation remains a
read-only record; a distinct nested publication record carries the exact prepared
Face/Show, effect, readiness and dispatch summary. Readiness also checks current
owner revision and pending delivery pressure. A planning record does not become
an available Show merely because preparation succeeded.

Added separately bounded canonical acknowledgement/interaction ABI entrances
(8 KiB JSON envelopes, canonical Fore encoding still checked independently).
Acknowledgement delegates to the full exact Show/effect check. Interaction reaches
only the context's exact scheduler node through Mask and ingress kernels and the
existing owner wrapper. The returned record distinguishes owner acceptance,
request transfer and transport completion; it is not a claim that an Editor or
Mask-change Host effect ran. One Show still permits one closing interaction.

The browser-runtime compile check passes. The composed CPU test now obtains its
initial publication from automatic Body preparation and passes. A focused session
command case passes on final source: unacknowledged input, stale acknowledgement,
exact acknowledgement, one owner selection, receipt visibility and replay refusal
without another state change. Host acknowledgement is a fixture; no DOM/QEMU,
broad suite, CI or artifact rebuild ran.

The default product planner/SDK does not yet select these chains or render/ack
this publication through its actual Host UI. Fresh-Show preparation after the
closing turn, complete Host-memory admission, legacy adapter retirement and
native Edit fulfillment remain unfinished. No production enactment or closure
claim follows from the new Rust/ABI path.

## Explicit fresh-Show replacement after a closing turn (#6201)

Added a bounded refresh entrance scoped to the current Body Plan, active Play,
Show, Face and revision. It prepares a successor from current authoritative owner
state with a checked next sequence, holding both publications' value leases,
then revokes/retires the old turn before replacing it. No owner command is replayed.
Stale replacement targets refuse before preparation; the new Show requires its
own genuine Host acknowledgement. Replacement preparation does not reseal Plans.

The latest dispatch receipt transfers without its request payload being cloned,
and retains its original Show/Face/revision beside the outcome. Observation can
therefore distinguish a prior accepted command from a newly prepared unacted Show.
This is one bounded latest receipt, not durable history or evidence that an
Editor/Mask-change effect was fulfilled. Pending requests remain in the existing
owner slot. A fresh pending view retains Source action contracts but marks them
unavailable specifically for owner-request pressure, rather than pretending the
return route is absent; Source values and request state are unchanged.

The focused resident command CPU case passes: selection, explicit replacement,
stale target/old acknowledgement refusal, new acknowledgement, typed Edit through
the second Show, and another replacement while preserving the pending Edit and
its original dispatch scope. Initial compilation found a borrowed test subject
spanning a mutable session call; made that client argument owned. An intermediate
assertion incorrectly expected unavailable contracts to be absent; corrected the
availability projection/reason and test. Negative logs remain local. No broad
suite, CI, DOM/QEMU or artifact rebuild ran.

SDK/Host rendering and planner selection, complete Host-memory admission, actual
Editor/Mask-change fulfillment, legacy adapter retirement and native Edit remain
unfinished. Host acknowledgement in these CPU cases is a fixture.

### Combined-dev reconciliation and browser transport

Synced the preserved Patchbay branch with omnibus #6203. Conflict resolution
retains protected native application-service ownership, source provenance and
pending-request checks before mutation. Native graph reading uses the existing
bounded hit regions rather than the newly overlapping inspection implementation.
Source spans and connection-origin classifications now serialize with the graph
for the protected service boundary; checked identity remains separate.

The browser runtime bridge, Host facade and SDK expose exact acknowledgement,
typed interaction and explicit refresh envelopes. Canonical requests retain the
Rust 8 KiB bound; legacy disposition remains 4 KiB. SDK responses still validate
Body Plan/Play scope, and Rust remains authoritative for admission. JavaScript
syntax checks pass. These transport methods do not render a Show, select a
canonical route, fulfill owner effects or retire legacy adapters. Platform proof
and CI campaigns remain deferred tonight.

### Canonical owner-effect dispatch

Canonical SDK submission is now asynchronous: after Rust accepts the exact typed
interaction, the Host invokes the same scoped `onPatchbayOwnerRequest` callback
used by legacy presentation. Both paths share a single owner/revision fence set
before awaiting the handler, so concurrent input and uncertain outcomes cannot
replay an effect. Valid reports use the existing checked disposition command;
handler failure reports Uncertain. Canonical dispatch does not enqueue a legacy
application observation to refresh its presentation; its Show replacement stays
explicit. No handler means the retained request remains pending.

The 21 browser Body Host CPU tests pass, including a new deferred-handler case
that submits concurrent canonical requests and verifies one handler invocation
and one disposition. JavaScript syntax and diff whitespace checks pass. This is
Host callback dispatch, not evidence that production canonical rendering/route
selection or native Editor/Mask-change fulfillment is complete.

### Production proposal and initial Mask resource admission

Workspace proposal now selects the two existing browser Mask recipes for each
local retained Patchbay resident from the current exact Host/Boot offer, before
sealing the authoritative Body Plan. Ordinary worksets get no added topology.
Missing offers refuse instead of using a synthesized offer or modifying a Play.
The shared selector also drives the existing canonical publication CPU fixture.

Browser Host acquisition includes captured initial Mask placements in its bounded
placement inventory and reserves their face slots. Rust Body start validates and
admits those initial placements in the same resource batch as workload
placements, through the installed membership advertisement. The standby chain is
not another active instance; a replacement needs its own admission. Preparation
rejects a chain order that would activate a different route than the one reserved.

An intermediate focused run correctly refused when both alternatives were
counted against the one-active-instance Back limit; corrected initial-only
admission rather than widening that limit. The canonical selection/Edit/refresh
CPU case then passed. The 22 Body Host CPU tests pass, including exact Mask slot
acquisition and duplicate placement refusal. Final initial-route refusal checks
are recorded in the issue checkpoint. This enables preparation through the normal
proposal path; actual DOM rendering/acknowledgement and legacy adapter retirement
remain unfinished. No browser/QEMU/CI campaign or release media rebuild ran.

Final focused Rust result: all ten `resident_` tests pass, including canonical
selection/Edit/refresh and rejection of a reversed initial Mask route. These are
CPU conformance checks; there is no new DOM or native product enactment claim.

### Selected canonical browser rendering

Canonical Patchbay publication now includes the existing normalized owner-Face
view, projected directly from the retained Mask Face/Show. Its interaction gate
also includes the owner's current revision and publication turn; an available
but consumed/stale Show cannot regain admission merely by rendering it.

The Host can select `onPatchbayPublication` for a prepared resident. It supplies
the already acquired initial Mask slot and fences that resident's legacy inline
canvas consumption. Handbook selects this path and uses the existing
`conduit-owner-face` component. Prepared content is rendered before exact Show
acknowledgement; hidden or disconnected components are not acknowledged. Typed
input reuses the SDK's checked acknowledged-Face argument projection, invokes the
existing owner-effect handler, explicitly refreshes the closing publication and
renders/acknowledges its successor. A refusal re-reads current Rust truth without
replaying the input. Callers without a selected canonical renderer keep fallback.

All 23 Body Host CPU tests pass, including a new selected-renderer case that
receives the exact admitted Mask root while invalid legacy canvas bytes are never
decoded. The focused Rust selection/Edit/refresh test passes with normalized view
checks before acknowledgement, after acknowledgement and after Show consumption.
JavaScript syntax and diff whitespace checks pass. An intermediate duplicate
parameter from option forwarding was caught by syntax checking and corrected.
No browser/QEMU/CI campaign or packaged artifact rebuild ran. Actual browser
DOM enactment, complete memory admission, native effects and full Source/legacy
adapter retirement remain open; this is source implementation plus CPU evidence.

### Route-specific actions and honest native Edit refusal

Handbook's canonical Patchbay renderer now marks only the three resident command
intents as supported. Body lifecycle or other contributors' actions remain
inspectable but cannot be submitted via this ingress. The SDK enforces the same
restriction from the current acknowledged Rust projection, never from a
caller-supplied replacement action. All seven Face-argument CPU tests pass,
including forged-intent and stale-Show refusal; JavaScript syntax and diff
whitespace checks pass.

Native Edit previously consumed its request and emitted only a checkpoint. It now
observes the current journey and records `native-source-editor-not-admitted` as a
FrontDoor action refusal, then uses the ordinary refresh to show it. Observation
precedes the refusal so a journey revision change cannot immediately clear it.
The helper is separate from the large controller; no keyboard sample, Source
mutation or editor success is invented. This corrects refusal behavior, not the
unfinished native Source editor; full document/editor admission remains required.

The branch now includes current dev through #6205/#6218. Layout merge resolution
keeps selected typography parameters and the new continuation-width regression.
The first in-flight compile encountered temporary merge markers; after resolving
them, the owning native-compositor library check passed on the resolved source.
No new native runtime or layout-execution result is claimed from that check.

### Fence resident legacy mutation and stop canonical inline graph production

The prepared browser Patchbay actor now selects its input mode from the exact
resident selector in the captured Body Plan. With a Body-composed canonical Mask,
nonempty legacy application events refuse with
`patchbay-canonical-interaction-required` before actor mutation or pending-request
handling. Empty observations remain a transitional bootstrap signal. Canonical
preparation selects the existing TextInspection delivery, so the transitional
application-view producer no longer constructs the inline graph payload. The
same retained actor and graph still supply the canonical Face. No canonical Mask
selector retains the existing legacy delivery/input behavior; unrelated resident
applications are unchanged.

A canonical Patchbay proposal now requires its selected renderer at Host
acquisition, before slots are allocated or Play starts. It cannot silently fall
back to inline controls whose return path is fenced. Handbook supplies that
renderer. Generic callers selecting a canonical topology must supply it too;
a built-in reusable canonical rendering default remains necessary for SDK parity.
This corrects the earlier broad statement that any caller without a callback
keeps fallback: fallback applies only to proposals without this canonical route.

All 23 Body Host CPU tests pass, including missing-renderer refusal before any
allocation/start. Ten focused resident Rust tests pass with the input fence;
the final selection/Edit/refresh test also passes after switching delivery,
checking no PatchbayCanvas after selection, unchanged revision on legacy input,
and normal canonical Show admission. Syntax/whitespace checks pass. Source/Fore
bootstrap retirement, generic renderer defaults, native editor effects, complete
memory admission and platform enactment remain open. No platform/CI campaign ran.

### Reusable SDK canonical renderer and bound startup

Canonical Patchbay now has an SDK default renderer for both ordinary Wake and
external Body preparation. It reuses the existing owner-Face component rather
than creating another widget implementation. Handbook's duplicated presentation
orchestration is removed. SDK packaging copies the canonical component and Look
stylesheet byte-for-byte and rewrites only the repository-relative import; no
runtime image or published artifact was rebuilt for this checkpoint.

Wake now binds the exact BrowserPlay before dispatching its first renderer
callback. External preparation binds the same way before exposing its Play.
Callbacks receive that Play directly, removing the application-variable race
where dispatch could start before `await body.wake()` assigned its result.
Custom renderers remain selectable. Low-level Host acquisition still requires a
selected renderer; ordinary SDK callers now get the reusable default automatically.

The default retains exact visible-DOM acknowledgement, checked typed input,
explicit successor refresh and no effect replay. Hidden retained Shows receive
one ResizeObserver visibility wake; it re-reads Rust truth and stops before
presentation, or disconnects on cancellation. It does not perform an action or
retry uncertain effects. Refusals display current truth in the admitted surface.

Validation: all SDK Plot tests pass including the first-dispatch Play-binding
regression; five default-renderer CPU tests pass, including visible/hidden Show
acknowledgement, refusal without replay, visibility wake and cancellation. The
23 Host and seven argument tests pass. The packaging fixture verifies identical
component/CSS bytes and local distribution imports. Syntax/whitespace checks
pass. This is CPU/packaging-fixture evidence, not actual browser enactment. Native
Source editing, complete admission, bootstrap retirement and platform evidence
remain open; no CI/QEMU/browser campaign or release archive update ran.

### Canonical presentation no longer waits for legacy output

Host dispatch now presents the already prepared canonical Patchbay publication
before draining ordinary effects. Exact captured resident selectors, current
Body/Plan/Play and the acquired Mask slot determine that path. A captured route
with no matching publication refuses; a renderer failure is not automatically
retried. Subsequent application-view output for that canonical resident is
ignored and cannot replace or re-acknowledge the Show. Legacy proposals without
a canonical selector retain their prior startup and rendering behavior.

All 24 Body Host CPU tests pass. The selected-renderer case runs both with an
invalid inline-canvas payload and with no legacy manifestation at all; both
render exactly once into the admitted Mask root. A separate legacy case forbids
canonical observation and still dispatches normally. Syntax/whitespace checks
pass. This removes the presentation dependency on bootstrap traffic; the old
Source event/view participants still exist and require typed Fore migration.
No browser/QEMU/CI campaign, artifact rebuild or platform enactment claim.

### Exact local custody before canonical owner dispatch

Source/Fore migration review found that the no-Line ingress could previously be
paired with a Mask Show on a different Host/Boot/generation. Publication
preparation, owner-turn binding and direct dispatch now share an exact local
custody check: the ingress fragment, producing Show and all Mask fragments must
name the same current Host/Boot/offer generation. A foreign source requires an
independently admitted Line; direct value copying cannot invent that connectivity.
Refusal occurs before owner access, pending-byte consumption or value-budget
admission, with `patchbay-local-ingress-host-mismatch` retained distinctly.

All six ingress-owner CPU tests pass, including independent Host, Boot and
generation mismatch cases with unchanged owner facts and retained pending bytes.
Positive fixtures now use a genuinely local ingress/Mask pairing rather than
implicitly passing a foreign one. The full canonical selection/Edit/refresh CPU
case also passes. Formatting/whitespace checks pass. This strengthens the current
transport seam; it does not complete Source/Fore bootstrap retirement, native
editing, complete storage admission or platform enactment. No platform/CI run.

### Canonical Source and resident root Fore (#6201)

Browser `plots/patchbay/main.conduit` now authors the existing typed
`FaceInteraction` input into `patchbay/accept-interaction`. The installed browser
catalog, exact finite offer and factory prepare the same retained Patchbay actor
at that actual sink node. Native deliberately retains the byte-identical former
Source as `native-retained.conduit`; its legacy 3 KiB route is not silently changed.

Canonical publication no longer prepares another ingress Plan/kernel per Show.
It borrows the root Plan's exact lowered local Fore and admitted value storage.
The root Fore stays open across successive acknowledged Shows; each Show remains
single-use. A monotonically increasing input sequence and exact owner-node/Host/
Boot/generation custody survive publication refresh. Only the matching owner Host
Call is dispatched; other pending calls remain undispatched. Finite root scheduling
settles the actual completed call before returning its sole input slot. Uncertain
admission/dispatch or terminal refusal cannot reopen the route for implicit replay.
The original detached ingress remains an explicit compatibility/test entrance.

The focused two-Show CPU case exposed and corrected retained-input slot pressure
on the second interaction. It now exercises canonical root selection, fresh Show,
Edit request retention, stale rejection and unchanged prepared value capacities.
The Host bridge's 24 tests pass, including canonical Source startup while waiting
for external input and no legacy initial application-view manifestation. Earlier
trials also found the closed-root fixture assumption and missing/extraneous Fore
queue limits; production planning now admits exact open-root bounds and inventory
expansion. These are CPU/bridge checks, not browser/QEMU or native Source migration
acceptance. #6192 native Source editing and #6201 platform enactment remain open.

### Native canonical document owner prerequisite (#6192, #6201)

The existing protected keyboard editor already uses shared `RetainedText`.
Command 14 adds canonical document read and revisioned replacement over those
same private bytes and revision, reusing the exact portable `TextDocument` and
`TextEdit` codecs. Legacy keyboard editing and canonical replacements share one
owner. Separate finite edit/document windows are 272/264 bytes; the legacy text
window and maximum document remain 256 bytes. The shared frame still has a
compile-time single-page bound. No global text-capacity increase is implied.

Root copies bounded values through the existing admitted region and current
owner binding. All eight typed replacement refusals remain distinct; malformed
mechanism responses and impossible successful revisions fault the region rather
than yielding invented success. No retry or second Root document is introduced.
The independently linked image's rebuild dependencies now include both the new
implementation and the shared canonical codec.

This is a protected implementation prerequisite, not a complete native editor.
The canonical state/face editor Backs, initialized-state/Runtime wire admission,
Mask editing realization and exact authorized Source handoff remain to implement.
Document-only admission must not inherit the legacy pipeline's serial Base grant:
reuse the region lifecycle while admitting only the actual selected ports/effects.
Explicit DATA Save/reopen belongs to #6192; namespace and cold-reboot durability
remain #6193/#6195. Patchbay's truthful unadmitted-editor refusal stays in place
until the real editor can be selected. No QEMU, CI or user enactment is claimed.

The freestanding x86_64 native-compositor library check passed for the canonical
owner/Root transport checkpoint. No protected runtime or QEMU enactment follows
from that compilation. The existing cold service builds completed in the same
original check; the final-source warm check also passed.

Document-only region preparation now omits the serial scope/capability altogether.
The legacy text/Morse paths retain their exact paired scope and handle. A serial
presentation without that explicit binding refuses before entering the effect
path. The common preparation validates the sealed Body Plan, current Host/Boot/
offer generation, and captures the exact Body Plan for activation; removing a
serial dependency must not remove those identity checks. More than one retained
editor in a partition refuses rather than aliasing its single private state slot.
Canonical document gears are recognized by the existing protected-region planning
and preparation selection, without advertising an unfinished state/editor Back.
The final freestanding x86_64 library check passes; no new test/proof campaign.

The checkpoint was synchronized with current `dev` (including #6220); the
post-sync freestanding native-compositor library check passed in 34.23 seconds.
The numeric Runtime document Back now prepares the canonical revision-zero empty
document before Play and emits it before any edit Host Call. Genuine edits reuse
the existing transform, bounded values, cancellation, and request sequence; the
initial publication does not consume request zero. Wire discriminant 6 is
additive; old receivers refuse it rather than reinterpret a legacy variant.

Two focused numeric regressions pass: initialized publication followed by an
actual admitted Host input, and wire identity round-trip. Initial fixture failures
were corrected (explicit pressure policy and fuel, and an admitted Host input
rather than unbudgeted remote ingress). No production remote Sign capacity was
added: native editor interaction admission still needs an explicit bounded path.
These checks prove kernel composition only, not guest execution or editor usability.

Canonical native state dispatch now maps the exact document implementation to
that Runtime Back and calls the existing protected `replace_document` operation.
Hosted compilation reuses the same shared revisioned text owner rather than
pretending keyboard events are canonical replacements. Refusal codes and details
preserve revision, encoding, range, UTF-8, capacity, revision exhaustion, storage
and mode distinctions. Both hosted and freestanding x86_64 native-compositor
library checks pass for this integration. The offer remains unadvertised: the
matching editor encounter, topology admission and checked Mask return path are
not yet complete. The native 256-byte capacity is unchanged.

### Canonical native Memory Lantern encounter (#6192, #6201)

The installed native Memory Lantern now uses its canonical `main.conduit`, the
same document/editor Source as the browser, rather than `keyboard-only.conduit`.
Its exact two-node initialized loop is admitted explicitly; other arbitrary
small or cyclic graphs are not newly admitted. The document Back retains the
existing protected owner, and `face/editor` has one bounded pending Host Call.
Physical default-keymap input returns real revisioned `TextEdit` values through
that planned call. The native profile appends Unicode, inserts newline, and
backspaces one Unicode scalar; arbitrary cursor selection and IME parity are not
claimed. The Mask displays an immutable returned document and never predicts a
successful edit. Empty input or a key release cannot masquerade as a document edit.

The editor reserves one existing keyboard delivery slot and its fixed adapter
memory; no remote ingress or remote Sign budget is invented. Document storage
remains 256 bytes. The full four-Plot workset is now 12 nodes/nine cords, within
the same fixed Runtime bounds. Cancellation retires pending calls, values and
presentation observations. Source loading has an admitted-call API which refuses
an existing nonempty draft and over-capacity Source, then checks the returned
revision and bytes; it does not replace resident Source or replan execution.

Validation at `62ff847f4`: 28 focused native-workset checks and nine resident
journey checks passed; the freestanding x86_64 native-compositor library check
passed. The first cold test compilation built dependencies, then subsequent
focused runs reused them. No QEMU, CI, ISO/ZIP, physical or human proof follows
from these checks. Native Patchbay request-to-Source authorization is the next
handoff; persistence and explicit Save/reopen remain #6192 work.

### Native Patchbay Source handoff (#6201)

`EditCurrent` now reaches the already-admitted canonical editor. The current
owner must first issue the exact request; a fixed-size digest and owner revision
retain its one-use authorization without another heap-owned request or document.
Graph membership alone grants nothing. Forged, replayed and stale requests refuse.
The handoff checks current Host/Boot/offer, Body Plan/Play, exact resident Source,
checked/expanded identities, unique subject and editor availability. It loads the
exact fitting Source via the existing pending editor Host Call, verifies the
owner-returned revision and bytes, records the actual result, then selects that
resident editor. Existing drafts and oversized Source refuse visibly. Fatal
execution errors retire the realization; no resident Source or running Plan is
silently replaced.

The receipt preserves Source/subject/Plan/Play provenance. Available compiler
spans are checked as UTF-8 bytes and converted to Unicode scalar coordinates.
`selection_enacted` is explicitly false: metadata does not claim native cursor
selection/highlighting. The current physical editor remains append/newline/
backspace, within 256 bytes. Explicit Save/reopen, Source publication/replanning,
and complete cursor editing are not claimed.

Ten resident journey checks passed including the actual owner-action handoff;
the focused handoff regression also passed after one-use authorization was added,
covering fabricated, replayed, stale and dirty-draft attempts with unchanged
Plan/Play identity. The integrated freestanding x86_64 native-compositor library
check passed at `2948f2491`. No new QEMU/CI or release acceptance is claimed.
The browser canonical root-Fore migration remains retained; native Patchbay's
legacy event/view Fore migration and platform enactment are still #6201 gaps.

### Protected Runtime custody and canonical ingress prerequisites (#6201)

The native boundary audit found a real gap beyond hosted editor execution:
Root's loan ledger seeded the legacy empty Edit output but did not seed the new
Document Back's eight-byte initialized snapshot. The ledger now grants exactly
that Root-known snapshot once on reviewed output routes. Genuine successor
Editor/Document values use existing exact completion loans. Two focused
specifications use real numeric kernel requests; a third covers canonical Fore
custody, wrong routes/bytes, duplicate sequences, pressure, revocation and the
absence of additional grants. All three passed. Compilation alone had not proved
this protected boundary; no QEMU enactment is retroactively claimed.

Canonical interactions require an explicit 8192-byte value specialization; the
legacy native value envelope remains 3072 bytes. The new Runtime Face profile
uses the same kernel and fixed 14 nodes/10 cords/10 value slots, with the same
24,576-byte aggregate native value budget. It has distinct InitFace/tag10 and
AdmitRemote/tag11 commands and typed Accepted/Full responses. Root uses a
separately sized broker and exact Fore custody receipts. Full stays live pressure
without advancing sequence or recording a loan; uncertain transport outcomes
quarantine rather than silently replay. Legacy/text/Morse ingress is refused.

This is a prepared migration prerequisite, not selection of a canonical native
Patchbay Back. The existing 3 KiB event/view route stays until its real owner,
Face publication, acknowledged Show return, authored Fore planning and finite
resource reservations all move together. A selecting offer must charge the actual
Face broker/service storage; the larger per-value type is not a global allowance.

The Face profile reserves 1024 mandatory remote Sign events in its existing
fixed backing; cord count is not an interaction count. The first ten-record
budget was corrected after inspecting the kernel's retained-event law. History
still has a finite terminal capacity; it is never silently replenished. Four
focused Runtime checks pass, including thirteen dispatched inputs on one
endpoint, exact 8192-byte ingress, typed Full/sequence behavior, wire refusal and
unchanged legacy storage. The hosted Runtime test harness uses an explicit
8 MiB thread because unoptimized copies overflowed its default test stack;
this does not change or prove the native 1 MiB stack. The linked native local
frame reservations are documented in the Runtime service README, with complete
peak stack/timing explicitly unproven. The integrated freestanding library
check passes; no QEMU, CI or release campaign was run.

The checkpoint was synchronized with `dev` through `63ac09331` (#6222/#6224)
without conflicts. The post-sync freestanding native-compositor library check
passed at `1ae49e2bc` in 7.56 seconds. The newly linked Runtime's three documented
local stack reservations were inspected again and remain unchanged; this is
partial static evidence, not complete peak-stack or guest-execution proof.

### Canonical Patchbay publication custody on the retained native owner

The selected native retained actor now projects a canonical `FaceFragment`,
retains the complete acknowledged Face/MaskShow, and consumes a typed
`FaceInteraction` against that evidence once. This reuses its existing graph,
selection and application revision. It adds no alternate Patchbay actor.
Rejected replacements revoke old publication authority; consumed Show sequences
cannot be republished. Typed semantic refusals preserve the actor and revision.
The service preflights the exact possible proposal reply before mutation.

Root independently retains the actual receipt and checks returned Edit/Mask
proposals against the genuine input and its selected graph/topology. Transport
uncertainty quarantines the service. A successfully published canonical slot
cannot subsequently accept the legacy event path. Legacy 3 KiB views and the
separate 8 KiB canonical input bound are unchanged; no aggregate heap or peak
native stack fit is claimed.

The focused application-core check passed three matching tests (query/codec,
typed refusal categories and existing exact graph-reply custody). The first
native Mask integration test correctly refused `PlayNotCurrent`: its initial
test setup supplied a born Body without an actual current playing Wake. This is
negative evidence, not an accepted interaction receipt; the test must bind real
lifecycle evidence rather than weakening Face validation.

This is owner/publication infrastructure for #6201 and #5994. The installed
native Source still selects `native-retained.conduit`; canonical Fore selection,
ordinary native publication/input dispatch and adapter retirement remain open.
No QEMU/browser campaign, CI acceptance, release or parent closure is claimed.

The corrected positive native Mask test now passes. Its Body, Wake, combined
BodyPlan and active Play are produced by the ordinary native planner/kernel and
canonical lifecycle transitions. The retained Patchbay actor receives the exact
current contribution basis; the complete publication crosses `encode_command`
and the existing workspace command decoder. Actual Mask scanout acknowledgement
then typed submission produces the exact Edit proposal and one actor revision
advance. Replaying the consumed Show and republishing it refuse without another
advance. This is deterministic CPU/native Mask evidence, not installed QEMU
execution. The canonical fragment query also uses an immutable owner accessor:
Root sends it through the same live service with transport sequencing and
quarantine, rather than caching another projection/actor truth.

Native Fore preparation is now explicit: the canonical Kind/catalog and Back
reserve one 8 KiB input, no output and one in-flight Host Call. Admission takes
the complete sealed Plan, validates the exact single closing-Flow Fore and sink,
and preserves actual fragment-set node/Cord/endpoint indices. It rejects Line
substitution, different contracts/bounds and unsupported activations. Fixed
Runtime bookkeeping remains in its existing tables; no guessed per-Back memory
allowance or detached scheduler is introduced. These helpers are prepared, not
yet selected by the installed Source.

Hosted native-compositor compilation and the final focused positive Mask test
passed. The freestanding compile caught missing exhaustive refusal-observation
arms for the new commands; those now carry explicit bounded operation names.
The subsequent freestanding x86_64 native-compositor compile passed. Final
complete-Plan helper changes receive the same focused compilation below; no
broader suite or product proof campaign is added.
The final complete-Plan Fore helper and Root owner wrapper compile together on
freestanding x86_64 (5.51 seconds, existing/preparation-only dead-code warnings).
The installed Source/Fore selection and ordinary dispatch remain unfinished;
this checkpoint does not close #6201 or #5994.

### Canonical resident Fore dispatch in the existing native Body scheduler

The native Play now prepares exactly one explicit legacy or Face scheduler
profile. Canonical selection depends on its actual admitted resident Fore;
legacy-only worksets keep the 3 KiB value profile. Both profiles retain existing
node/Cord/value-slot/aggregate bounds. The canonical dispatcher validates the
current Body/Plan/Play and acknowledged Mask Host/Boot/offer generation, enters
its exact Fore Cord, handles the actual selected owner Host Call, and drives its
completion before releasing input custody. Confirmed Full remains input
pressure; uncertain admission/completion stops the Play without replay.

The retained application owner now has a one-way canonical preparation command.
It preserves the actor, target and consumed-Show watermark, rejects legacy
ApplicationEvents, and omits legacy snapshot production for canonical slots.
Root independently retains this mode and rejects unexpected canonical snapshots.
Existing selection/topology controls refresh canonical state without completing
an invented legacy event-source request. Tutorial/Tour legacy paths remain
supported. Canonical initial actor construction selects the same existing owner
from the exact admitted implementation rather than creating another graph.

The Source preparation cache preserves authored Fore bindings for Patchbay;
other installed roots stay closed and all output boundaries remain unsupported.
The native planner can now plan and admit the canonical closing input Fore in
its resident partition. Its Kind/catalog, exact Back and resource reservations
are installed in the normal native preparation catalogs/advertisement.

Focused evidence: four application-core query/codec/refusal/mode checks pass;
the actual native Mask acknowledged Edit/replay test passes over real native
lifecycle evidence. A canonical Source planning test passes with an ordinary
native Host and combined fragment-set lowering: it preserves nonzero actual
node/Cord indices and refuses a foreign numeric sink. Final freestanding
x86_64 native-compositor compilation passes (4.57 seconds). These are local
conformance/compilation checks, not installed Source/scanout/QEMU acceptance.

Remaining before #6201/#5994 completion: connect ordinary ProductJourney and
FrontDoor/Mask publication/interaction handling, switch the installed Patchbay
Source, and retire its real legacy adapter consumers. The installed Source is
still `native-retained.conduit`; its replacement is deliberately not claimed
from these prepared paths. No new CI jobs, image/ZIP publication or platform
proof campaign was requested.

### 2026-10-22: installed native canonical Patchbay and ordinary Source editing

ConduitOS now selects the same canonical `plots/patchbay/main.conduit` Source as
the browser. Its actual resident Fore is planned, lowered and installed in the
existing Body kernel. Ordinary ProductJourney/FrontDoor observes the selected
retained actor's immutable fragment; Body membership owns resident declarations,
and native lifecycle actions remain source-owned alongside the Patchbay actions.
The ordinary admitted native Mask now automatically presents this surface.
Typed Mask submissions return through the actual resident Cord and owner Host
Call. Edit requests use the existing one-use authorization and real admitted
resident text editor; they preserve Body, BodyPlan and active Play identity.
Pending owner-request pressure disables further offered Patchbay actions.

The old event/view Source is now explicitly `fixtures/legacy-retained.conduit`
with its remaining browser regression callers named as fixtures. Installed
browser/native Patchbay do not select it. Generic retained-application support
for unrelated supported Sources remains. Native canonical preparation refuses
legacy events and creates no legacy view snapshot for the canonical slot.
The installed four-Plot workset now lowers to ten nodes and eight Cords (seven
internal connections plus its canonical Fore Cord), within unchanged bounds.

Canonical Face now retains all prior inspected Mask-stage facts as typed
properties: implementation, manifestation identifier, Host/Boot, resource pool
and class, reserved units, finite capacities and availability. Available stage
metadata is not an acknowledged Show or a resource-possession claim. Existing
Mask replanning and stale-request refusals exercise this canonical meaning.

Focused CPU results: all fifteen Patchbay-matching native tests pass, including
ordinary graph/Mask presentation, typed Edit into the actual Source editor,
consumed-Show replay refusal, Source authorization/draft preservation, exact
previous Plot/Plan observation, and graphical/speech Mask replanning. The two
specific native workset/input-count contracts also pass. After merging current
dev d72396eff (#6228), the installed typed-Edit path passes again. The focused
pending-request/draft test passes after the final pressure fix. Freestanding
x86_64 native-compositor compilation passes; no new QEMU/browser/CI campaign,
image/ZIP publication or stable acceptance is claimed.

Negative evidence: enabling the Face scheduler profile exposed hosted debug
constructor overflow on the default test-thread stack. GDB identified
`FixedScheduler::new_with_active_counts_and_host_calls` under the Face profile.
Hosted test preparation now uses the same explicit 8 MiB bounded constructor
fixture stack as runtime-core's existing Face tests, returning a boxed scheduler.
This is test-only; native stacks were not enlarged and native peak stack/heap fit
remain unproven. A richer BodyFace test projection also met the native producer
capacity refusal; regression helpers now use the actual ordinary FrontDoor Face
rather than claiming an alternate projection is installed. No bound or validator
was weakened. Editor Source remains limited to its admitted 256 bytes, and the
existing receipt still truthfully reports selection_enacted=false. Save/reopen,
real-QEMU/protected-service execution and parent acceptance remain separate gaps.

### 2026-10-22: choose an Edit subject without typing its identifier

For bounded graph subject sets, Patchbay now publishes the exact legal subject
membership in its existing checked text argument contract. This exposes values
the owner already required; it creates no widget Kind, renderer authority or
second selection model. Sets beyond the portable membership carrier retain the
existing bounded typed entry and owner membership check. Unknown values in the
finite case refuse as a typed constraint violation before owner effects.

The native Mask reuses its existing bounded text-choice buffer for provisional
command arguments as well as committed fields. Left/Right selects a draft from
the complete checked contract; Enter submits it through the same current Show
and resident Fore. No committed field value is inferred without its explicit
Source association. Existing 32-choice/2 KiB Mask limits remain unchanged;
unsupported/larger choice sets retain their complete contract and typed entry.

Navigation no longer executes command-argument drafts. Leaving their action,
opening reading/diagram/pages, or clicking background cancels the draft. Source
TextEntry fields retain their existing blur-commit behavior. Role and checked
contracts drive this mechanical distinction; no Patchbay action-name heuristic
was added to generic Mask input, and Look does not own it.

Focused local results: five resident canonical command/refusal tests pass; nine
native text-field/choice tests pass, including draft-only browsing and reading/
pointer cancellation. The existing text-field Tab-commit/stale-Show regression
passes. The ordinary installed Patchbay keyboard choice -> typed Edit -> actual
Source editor path passes, including Tab cancellation before explicit Enter.
Current dev 624baf562 (#6225) is merged, and freestanding compilation of the
combined source passes. This remains CPU enactment/compilation; native stack,
private heap, QEMU/protected-service, browser/platform and release acceptance
are not inferred from these checks.
