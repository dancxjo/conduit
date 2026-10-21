# Reopened Crèche page flow, authoritative scope refresh

Live #6158 is open. The Oct20 22:41UTC reopening preserves #6162's narrower
orphaned-field fix and adds a distinct remaining page-flow defect. Original goal
step2 is therefore incomplete; #5990 and #5607 stay closed at their proven scope.

The current panel allocator assigns each of the first two authored regions a
fixed lane on every page. Its counters for rows and graphics commands are global
per page. When the right region exhausts a page after the left region has ended,
`Flow::next_page` restarts the same right lane, leaving an empty left half. This
is a concrete mechanical explanation for the reported geometry, but not proof
of the unknown user image/source/framebuffer identity.

Preserve the 20-row/48-command limits and field cohesion. A wider framebuffer
alone cannot erase independent row or command pressure. Measure and record those
three causes separately in a real current normal-product reproduction with all
five choices, four selected, and the unavailable reason.

Bounded candidate: a continuation page containing only the remaining one of the
two authored regions should use a single full-width continuation panel. Keep the
same source rows, stable meaningful order and exact action/argument identities.
Do not reserve an empty lane merely because the region previously occupied the
right column. Recompute wrap, field-cohesion demand, row/command costs, panel
bounds and hit geometry together after selecting that page's width. A coordinate
translation alone only moves the wasted half and is insufficient. No general
CSS/grid engine, Source reparenting, new runtime truth or budget increase.

First establish deterministic source-bound regression for the exact five-choice
shape and command/row/height pressure, including oversized Unicode fields and
narrow collapse. Then current normal product wide/narrow QEMU must show complete
choice/review/Birth context, exact keyboard and pointer reachability, fresh Show,
and explain any remaining required continuation. Do not close from a differently
sized or cropped screenshot. The existing selected-typography reconciliation is
frozen on origin while focused checks run; do not silently fold this repair into
that change. Serial delivery remains through #5994; broader #6159 stays separate.
