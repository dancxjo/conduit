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
