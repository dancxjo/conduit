# Compatible difference roles in atan2

This development slice addresses the compatible-role requirement in #5392.
The existing exact `atan2` denotation and bounded writer admit two Linear
quantities or two Delta quantities. Points and mixed roles still refuse.
Dimension and family compatibility, principal branch rules, finite work,
source-evidence validation and caller-buffer bounds remain in the existing
implementations.

Temperature differences use scale-only conversion. In particular, `9°F` as a
difference equals `5K` as a difference, giving an exact `1/8turn` bearing.
Celsius and Fahrenheit point offsets cannot enter this calculation. The
original quantities and literal evidence remain in the prepared denotation.
The negative x axis keeps the principal `+1/2turn` result.

The Plot intrinsic also recognizes the existing intrinsic `TemperatureDelta`
Type. This is local admission to `math/atan2`; it creates no global nominal
subtyping or new constructor syntax. Its ordinary intrinsic leaf validation
rejects point and wrong-family bytes. Generic `Quantity` inputs retain their
runtime role and dimension checks.

Core conformance covers all axes and quadrants, exact scale conversion,
enclosure equivalence to the same linear ratio, forged source evidence,
point/mixed-role refusal, zero-pair refusal and untouched caller buffers.
The typed Plot regression first reproduced `CND-FRM-046` on the authored
`TemperatureDelta` record. Its repaired reference and prepared evaluators
produce independently specified exact turn fractions.

Installed Std tests execute checked Source through ordinary expansion,
planning, sealed Plan installation and the selected
`std/kernel-pure-expression@1` Back. Both generic Quantity and intrinsic
TemperatureDelta inputs produce exact turns with zero observed allocations
after Play starts and unchanged Value capacity. Generic inputs with points or
mixed roles refuse with HostCallFailed detail 21, a zero pair with detail 22,
and mismatched dimensions with detail 20. No output escapes these refusals;
a subsequent valid input succeeds. The failing public execution API does not
expose an allocation report, so these failures have no zero-allocation claim.

The separate CLI regression uses the existing
`units/compare-temperature-differences` receipt as its typed source, preserving
the authored Fahrenheit and Kelvin units. It verifies the selected bearing
Back, Source identity, Plan integrity, one correlated Play, Completed terminal
observation and exact Face output. This CLI regression passes; the old
digest-bound CLI reproduces the typed-Source refusal with the same authored
fixture. A separate retained CLI run has a complete report with one Plan and
one Completed Play. An initial test assertion incorrectly assumed a Gear ID
format after all numerical and terminal assertions passed; the final assertion
inspects the selected canonical `math/atan2` program instead.

Local validation includes 13 Core conformance tests, seven Plot tests (two new
intrinsic-role and five existing inverse-angle cases), three installed Std
tests and the actual CLI regression. Strict Plot/Std library and test Clippy,
strict CLI regression Clippy, Core without default features and formatting pass. These checks are scoped
local evidence, not a full workspace or combined-source CI pass.

This work remains private preparation while the Fore contract PR is held.
Fresh combined-source CI, full integration and required publication remain
necessary before stable acceptance. It does not complete #5392, composite
approximation, a general symbolic numeric consumer, browser execution,
installed no_std/device proof, or a timing/WCET claim.
