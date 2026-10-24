# Compiled input-node locations

Eligible constructor children selecting fixed canonical input geometry now
compile to finite input-node copy instructions. Preparation resolves record
fields, tuple fields and constant collection indices, including fixed nominal
representations. No selection evaluator, field-name String, output Type prefix
or intermediate result is retained for those children. Variable sequences,
variants and unsupported leaves keep ordinary selection; optimization limits
are not new program-refusal rules.

The enclosing evaluator still validates the complete canonical input and exact
Type, its input envelope and boundary predicates before executing these private
instructions. Copies preserve the entire selected value node, including framing.
No quantization, scalar conversion, effect, routing or authority is introduced.
Instruction and constant vectors still own RAM; this is not flash lowering or
whole-graph lifetime allocation.

The 64-bit rounded retained packet model falls from 8,608 to 4,576 bytes
(previously 26,912 before the literal/constructor changes). All 512 actual
packet-program cases and 256 nested record/tuple/collection/nominal cases match
the reference with zero recurring allocation. Negative checks cover every
truncation of a valid input, malformed Boolean, foreign exact Type and incorrect
projection Type. Variable-length preceding fields vary at runtime without
assuming an offset. The packet regression ceiling is now 5 KiB in this CPU model.

Fifty-three focused tests pass across expression conformance and actual ordinary
sampler/output kernels, including the finite CPU overnight workload, sensor
outcomes, output refusal, cancellation and native assignment custody. Scoped
strict Clippy, thumbv6m no_std check and native-Face ESP32 release link pass.
No firmware was flashed. The physical Root captures in the ledger predate this
change; neither their heap delta nor combined MCU admission is claimed here.
