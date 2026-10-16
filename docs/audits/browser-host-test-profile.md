# Browser Host CI test profile

The Browser Host unit shard uses the existing level-one test optimization
profile, preserving debug assertions, overflow checks, its complete package
and portable-proof selection, and its current timeout. The separate packaged
Chromium product gate is unchanged.

On development base `2dd715bf3129e115bcd0d0ba743578224ca03bf8`, the complete
`cargo xtask ci pipeline unit hosts-browser` command passes 486 tests across
12 harnesses, with zero failures and the same two existing ignored tests.
This includes the native browser packages, portable clock library proof and
28 portable USB HID tests. The main runtime harness reports 80.01 seconds.
Assertions and overflow checks remain enabled. Pinned actionlint and all 220
pipeline contract tests also pass.

This is hosted native/software proof. It neither establishes a controlled
cold-CI speedup nor substitutes for packaged Chromium or physical USB proof.
