# Prepared expression baseline (#5539)

Run `cargo xtask check expression-baseline`. The check emits immutable
`EXPRESSION_BASELINE_ROW` JSON records; retain its stdout with the build/source
revision when comparing runs. This is report-only: no speed threshold or
optimization is introduced.

The actual checked fixture is
`architecture/plot/tests/fixtures/expression_baseline.conduit`. Each unsigned
64-bit expression sums 2/4/9/33 copies of its input using exactly 1/3/8/32
addition operators. Balanced trees keep the programs within the existing
16-level decoder budget. The left-associated 32-operator expression checks but
refuses canonical preparation (`CND-FRM-046`); the validator currently wraps
that malformed program-depth refusal in its generic construction diagnostic.
The audit does not widen that budget or silently optimize a deep tree.

Three rows per size retain Source/checked/expanded identities and separate full
corpus parse/check, selected-Plot expansion, program decode/preparation, warm
prepared evaluation plus byte assertion, and native checked-add loop plus
assertion. Reference and prepared outputs match exact little-endian U64;
overflow and malformed input refuse. An explicit untimed evaluation precedes
warm timing. Allocation observation is a separate untimed loop and proves zero
evaluation allocations/reallocations for this primitive profile. There is no
kernel Start in this measurement and no claim about all allocations in Play.

The prepared evaluator owns boxed operation nodes and fixed output storage;
evaluation recursively visits that existing tree. It is not an instruction
tape, JIT or fused execution. Native timings use the same build's test profile;
this is not a release-optimized performance claim. Structured-value transport,
Host Calls, queueing, Sign and production kernel costs remain #5536/#5537.

## Host boundary

The std dynamic Back is `std/kernel-pure-expression@1` under
`std/pure-expression-kernel-hosted@1`, built from the exact program by
`targets/std/offers/src/pure_expression.rs`. The browser dynamic Back is
`browser/kernel-pure-expression@1`; its preparation validates the selected
placement and uses the same prepared evaluator. ConduitOS offers
`conduitos/kernel-pure-expression@1` under
`conduitos/pure-expression-cooperative-bounded@1`: exact input/output frame
bounds and at most 16 active instances. Operator count is not instance count.

These are implementation/eligibility mappings from current code, not browser
or firmware timing receipts. A Host lacking the exact generated Kind/Back or
its required Host Call cannot use the generic evaluator's availability as an
offer. A no_std build still needs allocation during preparation; a profile with
no such admitted preparation storage is unsupported. Missing/changed selected
placement, malformed program, and over-depth programs refuse. Keep browser,
QEMU/device execution and performance claims separate until those owners
provide their exact receipts.
