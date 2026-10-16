# Exact Angle reference transport

The development proof enters through `cargo xtask check angle-reference`.
It checks the explicit `1turn = 2π rad` reference law, checked Source,
prepared expression storage, exact numerical provider selection and installed
kernel execution. Stable acceptance still requires the normal release train.

`math/radians(angle)` projects a turn-origin Angle into its exact symbolic
radian coordinate. It accepts degrees and exact declared turn scales, and
returns `MathematicalValue`. It never attaches Angle to a bare scalar or
rounds π into a rational Unit scale. The first profile refuses unrelated
origins, including an already radian-origin input; it does not invent a
general bridge between arbitrary physical reference origins.

The original Angle remains in the checked Source and can be retained beside
the projected coordinate in an ordinary record:

```conduit
type Observation = {
    angle: Angle
    radians: MathematicalValue
}
plot observe (
    >> angle: Angle
    result: Observation >>
) = ({angle: ., radians: math/radians(.)})
```

Limited numerical representation is a separate operation. A checked
`math/realize` request selects a std numerical Back with a source-specific
certificate. For a symbolic value, the installed finite profile encloses π
with the existing exact series, rounds a rational midpoint to binary64, and
bounds the distance from that machine value to both interval endpoints.
The result retains the original symbolic source, target bits, rounding mode,
certified total error and the selected implementation/artifact provenance.
Its arithmetic rounding flags describe the midpoint rounding phase;
the total source error is the separately certified bound.

Enclosure terms, arithmetic work and preparation storage are finite. A
precision request that the profile cannot certify refuses before Play.
There is no automatic refinement retry or unrecorded floating-point cast.
Play emits the already prepared receipt using the existing numerical Back.
The older exact-rational and binary64 certificate encodings remain unchanged.
