# Exact inverse Angles

Run `cargo xtask check inverse-angle` for the complete development proof.
The suite uses optimized tests with debug assertions and overflow checks enabled.
Stable acceptance still requires the normal candidate and integration gates.

`math/atan`, `math/asin` and `math/acos` accept exact rational
`MathematicalValue` arguments and return `MathematicalAngle`. Their principal
branches are part of their reviewed meaning: atan is between -1/4 and +1/4
turn, asin uses [-1/4,+1/4], and acos [0,1/2]. Unsupported symbolic argument
proofs and out-of-domain asin/acos inputs refuse.

`math/atan2(y,x)` accepts same-Dimension typed Quantities, converts their
immutable Unit coordinates exactly, and returns the principal signed branch
(-1/2,+1/2] in turns. The negative x-axis selects +1/2. There are no implicit
scalar radians. `math/atan2(3m,400cm)` admits; `math/atan2(3m,4s)` refuses.
Zero pairs refuse as undefined, including the unit-independent literal (0,0).

```conduit
type LengthPair = {
    y: Distance
    x: Distance
}
plot orientation (
    >> value: LengthPair
    result: MathematicalAngle >>
) = (math/atan2(.y, .x))
```

MathematicalAngle pairs the existing canonical turn Unit with an exact
mathematical-coordinate DAG. It is a numerical representation of Angle;
it does not change the existing decimal Quantity codec or add a Unit law.
Typed ports do not silently narrow it or discard its Unit. Prepared execution
writes into admitted storage and retains named domain refusals.

Approximation is a separate preparation operation. The ordinary mathematical
enclosure entrance retains the exact graph and certifies the full requested
turn-coordinate error, including root and arctangent uncertainty for asin/acos.
Work, terms, refinements and storage are finite. An insufficient profile refuses;
there is no hidden retry or host floating-point shortcut. Installed proof uses
the ordinary Std Host, sealed Plan and production kernel, not physical hardware.
