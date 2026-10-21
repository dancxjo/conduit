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
