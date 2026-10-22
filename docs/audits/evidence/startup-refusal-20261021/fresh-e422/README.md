# Startup refusal checks on refreshed dev

Clean tested Source: `e422ea7323a4804a5315df4e4d1f6c18fbf9a2c9`, including
reviewed dev `f3e582ce98ede25394a392de4ed06e0b6b086133`.

All 26 controls pass: one exact replaced-Show/stale-owner test, six startup,
eight lifecycle, six journey, four Birth and one exact forged/failed-Show test.
Strict hosted-tools/native-compositor all-target Clippy and full workspace
formatting pass. The runner asserts RUST_MIN_STACK is unset; no stack limit
increase or test weakening. This reruns the corrected test fixture at the fresh
combined Source, preserving both sequential refusal checks and projection
assertions. Older stack overflow and diagnostic packets remain unchanged.

Each command, original log size/hash, test count and clean Source is retained
in validation.json. Original logs are losslessly compressed. Root rehashed all
eight originals and independently checked each test count. The runner is
retained to bind the default stack assertion and build profile.

Four debug-profile jobs used the existing shared cache under Cargo locking
while a foreign release-profile cross-build occupied disjoint outputs. Elapsed
command values are operational receipts, not performance measurements.
This is hosted production-handler/fixture evidence, not installed guest fault
injection. Earlier native compilation remains bound to its own Source. No new
QEMU, Candidate, Integration, publication, physical or full issue acceptance
claim. This documentary successor changes no tested implementation.
