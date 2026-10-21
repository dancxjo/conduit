# Patchbay bounded input plan

Source inspection on 2026-10-21. Implementation remains queued behind the selected-Look native scene change.

Current diagram geometry exposes at most six Gear card hits; actual rendered links are capped at twelve and ports at four per card. Complete Details already resolves exact Gear/Port/Cord subjects against current Face identity and revision. The guest has no inspect semantic action: do not fabricate one.

Reuse the portable FaceNavigation / NavigationOperation::FocusAndDisclose contract, with an explicitly admitted focusable subject list derived from the exact graph projection. Keep navigation separate from owner invocation. Establish one bounded preparation helper rather than enlarging the input reducer. Validate the navigation identity and current Face revision before resolving a selected subject.

Derive drawing and pointer targets from one geometry traversal: exact visible Port row rectangles, exact Gear header/card regions and actual Cord polyline segments. Resolve Port rows before Gear card backgrounds; resolve uncovered path segments with a finite distance tolerance and deterministic tie order. Do not make omitted/ineligible links clickable. Store exact Face subject references or indices, never a second topology. No additional graphics command is necessary merely to register a hit.

Keyboard traversal must cover all advertised Gear/Port/Cord identities, including facts omitted from the diagram; selecting an omitted subject opens its existing complete Details page. Visible digit shortcuts remain conveniences, not the only access path. Retain page continuation and a clear return to diagram.

Focused proof must cover both endpoint directions, overlapping hit priority, off-page/omitted Cord access, full labels/provenance, stale Face/Show refusal, missing subjects and unchanged owner state. Then actual QEMU proof and common Face browser/linear correspondence belong to #5518/#5519. Respect 48-command/192-byte/20-row/8-surface limits.
