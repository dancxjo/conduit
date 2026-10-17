# Prepared numerical receipts

Run `conduit run plots/numerics/main.conduit --report numerics-report.json`
on the installed std Host. The report retains both typed Face Signs. Both
requests retain exact mathematical source `0.3`. Zero error requires the rational
point result; the loose request admits a finite binary64 point whose exact absolute error is
`1 / 90071992547409920`. Its receipt retains the IEEE bits, rounding direction
and operation status alongside the mathematical source.

The source names meaning and precision. The Host prepares source-specific offers,
the ordinary planner selects them, and the immutable Plan retains their checked
certificates. The kernel emits frozen typed receipts. Faces retain source,
endpoints, actual error and representation; they do not claim an interval is an
exact decimal value.

Product acceptance runs through `cargo xtask ci pipeline unit products`. The
`numerical_realization` product tests inspect actual Plan and Face Signs;
std offer tests cover generation, repeat preparation and transactional refusal.
This development slice covers rational, outward-integer and prepared finite
binary64 point realizations. Irrational runtime strategies and angle/trig remain
open. Arithmetic-program float certificates retain intermediate rounding in the
Plan property; native dynamic DSP/tensor execution remains a separate gate.
