# Borrow linked observation and publication schemas

The generic Core composer accepts an immutable static schema only after checking
it against the complete exact Type supplied independently at preparation. Child
schema/name ranges then borrow that prefix. The ordinary constructor still owns
its schema; both paths retain identical canonical output checks and bounded
mutable output storage. Foreign/truncated schemas and insufficient envelopes
refuse before Play.

The environmental observation codec now borrows its linked decoder input Type
and its temperature, humidity, observation and result Types. Publication
validators borrow the independently checked Type for each channel. Build
preparation emits the quantity field Types from checked Source; no Type encoding
or byte offset is handwritten. Quantity Forms and semantics remain unchanged.

On the same 64-bit sampler fixture this reduces retained initialized software
from 50,914 to 48,483 bytes (2,431 bytes). Preparation peak remains 86,750 bytes.
This is not an MCU heap measurement or a whole appliance total. All 513 Core
tests and 23 appliance/quantity/publication tests pass, including the full
960-sample finite workload and recurring-allocation probes. Cortex-M0 Core
checking and strict library linting are recorded alongside these results.
Physical captures remain historical until a separately recorded native run.
