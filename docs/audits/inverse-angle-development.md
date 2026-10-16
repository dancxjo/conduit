# Inverse-angle development

The local #5734 candidate implements exact principal inverse denotations,
certified numerical preparation, source-correlated typed atan2, and exact Angle
transport. Authored inverse-call lowering, installed runtime integration and
exact-head acceptance remain pending. This is not complete issue acceptance.

Arctangent retains its exact rational input and returns a certified interval in
turns. Reciprocal and quarter-turn identities reduce its series argument to
magnitude at most one half. The alternating series retains the first omitted
term as a rigorous remainder, sharing the arithmetic used by the Machin π
enclosure. Division by an independently enclosed positive 2π produces turn
coordinates. Terms, limb work and final width are admitted bounds; insufficient
profiles refuse. No host floating-point evaluator supplies the certificate.

Principal asin retains a certified interval for sqrt(1-x²), then applies
monotone arctangent endpoint bounds. Negative inputs reverse the division
endpoints. Acos subtracts the complete asin interval from one quarter turn.
The final error includes root and both arctangent bounds. Outside-domain,
unresolved positive root, exhausted refinement and excessive total width
remain distinct refusals. Exact zero and endpoints need no root or series.
The half-input test profile retains a 1/10^8-turn error bound, admits one billion
limb-work units and observes about 175–178 million; its earlier 100-million
budget refused. These are arithmetic-work observations, not device timing.

Exact ArctangentTurns, ArcsineTurns and ArccosineTurns DAG nodes share principal
domain and rational-identity checks between builder and borrowed codec.
Unproved symbolic arguments refuse. Canonical encoding preserves function
identity; ordinary mathematical-enclosure projection retains the exact graph.
A numerical enclosure never silently replaces the authored denotation.

QuantityAtan2Denotation checks both original Quantity spellings, explicit
common Unit and principal signed-half-turn branch before constructing an exact
coordinate expression. Mismatched Dimensions, nonlinear roles, forged source
and the zero pair refuse. Metre and centimetre common coordinates give the same
exact result. The negative x-axis selects +1/2 turn. PreparedQuantityAtan2 then
projects that same graph through the ordinary enclosure entrance, retaining
original conversion receipts and typed rational Angle bounds. Refusing a
numeric profile leaves the independently prepared exact denotation intact.

MathematicalAngle is a numerical representation using the existing Angle
Dimension, linear role, canonical turn Unit and mathematical-coordinate DAG.
It introduces no Unit definition and leaves the decimal Quantity/Angle codec
unchanged. Its versioned codec includes the existing Unit descriptor. Borrowed
Info validation rejects foreign Units, unsupported versions and malformed
coordinates. Plot Type bounds, prepared storage and native carriers retain
this finite representation. There is no implicit narrowing or Unit erasure.

Checked Plot pack/unpack tests retain an exact inverse coordinate across typed
structured ports. The allocation probe observes zero allocations and zero
reallocations over 100 repeated prepared executions. Both evaluators apply the
existing mathematical proof relation to Angle equality: unknown remains an
explicit refusal. Native carrier round-trip and forged-unit refusal are tested.
