These are manual component proof receipts, not Cargo-produced fingerprints or
firmware/admission receipts. `summary.json` states the qualification boundary.
`commands.json` retains exact compiler commands, features and reused immutable
OUT_DIR Forms/schema hashes. Current changed resident inventories were regenerated.
`rlib-sha256.json` pins the actual final component dependencies; `source-sha256.json`
pins changed source. Qualifier scripts are cache-bound reference commands, not a
new build framework or a replacement for regular Cargo/Make/target gates. They
reuse only this lane's original warm cache and require those artifacts to exist.

The source-capture test compiles the actual existing module/tests directly.
The zero-inventory harness compiles the exact existing generated-source fixture
and owning tests. Kernel/MCU workspace tests are original owning tests, compiled
from unchanged sources; no Body approval or lawful Source Start is inferred.

Stored text logs normalize trailing blank lines; raw output remains in the lane's
qualification cache. Test result lines and measurements are unchanged.
