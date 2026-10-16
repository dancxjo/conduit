# Inverse-angle preparation development

The current development slice for #5734 implements principal arctangent
enclosures and source-correlated same-Dimension `atan2` preparation. It does
not yet supply asin/acos or an authored Plot/runtime entrance; this is not
complete issue acceptance.

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
