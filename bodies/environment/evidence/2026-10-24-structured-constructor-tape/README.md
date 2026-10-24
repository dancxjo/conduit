# Linear structured constructor execution

Records, tuples, collections, variants and conditional output branches compile
to bounded forward-only encoding operations. Adjacent constant headers, tags,
field names and Boolean/octet material coalesce into a byte pool. A single
parent output buffer receives the result; eligible nested constructors retain
no child evaluator, Type prefix, field-name String or intermediate output.
Dynamic leaf expressions and checked selections retain the shared evaluator.

The compiler bounds operations, dynamic values, constant bytes and depth.
An ineligible or over-bound optimization falls back to ordinary preparation;
it does not become a new program refusal. Existing constructor-law validation,
conditional laziness, exact canonical bytes, input bounds and output predicates
remain applicable. The private compiler emits only forward branches, so Play
introduces no unbounded instruction loop or allocation.

All 27 focused tests pass. They include 512 actual packet-program cases,
256 padded-record inputs, lazy failure/reuse, tuple encoding, invalid duplicate
fields, a constructor exceeding tape value capacity, deep valid Types and
constructed outputs larger than their input envelope. Strict library/test
Clippy passes. The 64-bit allocation model drops packet evaluation backing
from 18,688 to 8,608 bytes and literal padding from 2,592 to 1,088. Earlier
pre-literal-cache packet backing was 26,912 bytes. These are retained allocation
figures rounded to 32-byte units, not MCU heap bounds.

The native-Face ESP32 release image links. No firmware was flashed for this
change. Scalar evaluators, input-member selection and some intermediate
workspaces remain. Instruction/constant vectors still own prepared heap
storage; linked flash material and whole-graph lifetime allocation remain
separate work. This is not combined MCU admission or a new Mask/Host grant.
