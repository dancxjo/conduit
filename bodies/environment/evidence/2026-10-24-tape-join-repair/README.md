# Preserve common constructor bytes at conditional joins

Constant-pool adjacency is insufficient for instruction coalescing. After a
conditional, extending the false branch’s last Bytes instruction can hide a
following common record header from the true branch. Coalescing now stops when
the next instruction address is a branch target. The next common bytes get
their own instruction; both paths execute it. This is preparation-only logic.

A new 256-input regression checks a conditional collection followed by another
record field. The formerly failing BME lifecycle and binary-frame concatenation
cases pass. The full Plot suite on the PR branch passes 876 tests across 100
targets, plus strict scoped Clippy. That branch excludes the subsequently held
fixed-input-location optimization, so this is not a whole-construction-tree
integration claim.

On the construction branch, 68 focused tests pass across expressions, the
join regressions, the ordinary sampler (including its CPU overnight transcript)
and assigned output/refusal/cancellation. The fixed-input packet model remains
4,576 retained bytes on the rounded 64-bit allocator measurement, with no
recurring allocation. ESP32 native-Face release link and thumbv6m no_std checks
pass after the repair. No firmware was flashed and no MCU heap delta or
combined admission is inferred. Existing pre-repair evidence is preserved.
