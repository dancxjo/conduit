# Inverse-angle preparation development

The current development slice for #5734 implements principal arctangent
enclosures, source-correlated same-Dimension `atan2`, and principal asin/acos
preparation. It does not yet supply an authored Plot/runtime entrance; this
is not complete issue acceptance.

Arctangent retains its exact rational input and returns a rational interval
in turns. Reciprocal and quarter-turn identities reduce the series argument
to magnitude at most one half. The alternating series retains the first
omitted term as a rigorous remainder. This is the same arithmetic used by
the existing Machin π enclosure; there is no host floating-point evaluator.
Division by an independently enclosed positive 2π converts the certified
series interval to turns. The final width is checked against the admitted
turn-coordinate tolerance, separately from the series tolerance. Terms and
limb work are finite; inability to meet the profile refuses explicitly.

`PreparedQuantityAtan2` requires checked original Quantity spellings, a common
Unit and its exact spelling, and an explicit principal signed-half-turn
branch. Both input conversions retain their source receipts. Mismatched
Dimensions, nonlinear roles, forged source evidence and the undefined zero
pair refuse. Successful endpoints are typed Angle Quantities with Unit turn.
The negative x-axis selects positive one half turn; other quadrant offsets
are exact. Input units are never treated as scalar radians.

Core tests cover independent numerical brackets, exact cardinal/axis cases,
all quadrants, `3m`/`400cm` conversion parity, refusal and finite-work limits.
The existing π and forward-trig regressions remain necessary after sharing
the series implementation. Authored-source integration, further inverse
branches and exact-head admission remain pending.

Principal asin retains a certified square-root interval for `sqrt(1-x²)` and
uses monotone arctangent endpoint bounds. Negative inputs reverse division
endpoint order. Acos subtracts that complete interval from one quarter turn.
The final width includes root and both arctangent bounds; inadequate root
resolution or excessive combined angular width refuses. Inputs outside
[-1,1] have a separate domain refusal. Exact zero and endpoints require no
root or series work.

The half-input regression profile admits one billion charged limb-work units,
root width 1/10^12, and final angular width 1/10^8 turn. Observed preparation
work is approximately 175–178 million units across the signed asin/acos cases.
These are internal arithmetic-work observations, not device timing or a
runtime speed claim. The earlier 100-million-unit fixture budget refused;
the error bound was retained when admitting the larger preparation budget.

Exact principal inverse denotations are now canonical mathematical DAG nodes:
`ArctangentTurns`, `ArcsineTurns` and `ArccosineTurns`. Builder and borrowed
codec validation share domain and rational-identity checks. Existing node tags
and encodings remain unchanged; the three additional tags preserve inverse
operation identity across encoding, decoding and source-bound enclosure.
Only rational arguments are currently admitted for these inverse nodes;
unproved symbolic argument domains refuse. Principal endpoints and half-input
asin/acos identities reduce exactly before numerical projection.

The ordinary mathematical-enclosure entrance uses the certified inverse
preparations for these nodes. Its leaf-width, term, root-refinement, storage
and limb-work requirements remain authoritative. The projected record retains
the original exact graph, and an impossible zero-width request refuses rather
than replacing the graph with a point approximation. The typed Angle envelope
and authored/runtime entrance are still separate pending integration work.
