# Native local implementation checkpoint

Tonight's user instruction defers proof/CI work in favor of implementation.
This branch includes current `dev` (1e056b899), the selected typography work,
and the four-line obsolete lint-expectation removal. No new PR or CI queue is
created by this checkpoint; no stable acceptance or issue closure is claimed.

## #6158 continuation

After the first authored region is exhausted, continuation of the remaining
region occupies the available panel width. Page arrangement precedes field and
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

## Coordination

Origin also contains `work/native-look-local` with overlapping Patchbay work
and separate forced-color/fallback changes. It is preserved untouched. Reconcile
those reviewed changes before admitting either branch; do not stack two local
selection models. The present branch uses the portable navigation contract and
same-traversal hit collection. #6190's required-item admission remains separate.
