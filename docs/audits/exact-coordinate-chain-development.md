# Exact coordinate-chain development evidence

This records development evidence for #5392. The [retained receipt](evidence/exact-coordinate-chain-development-20261020.json) identifies the tested source, executable digests, cases and proof scope. The original parent requirement matrix remains authoritative.

The checked Plot connects two installed Gears: `math/coordinates` performs an exact Unit-coordinate change, then `math/near` consumes that exact result under its own numerical allowance. Each boundary emits a separately replayable general projection report. Coordinate evidence retains the complete original Quantity wire and selected Unit capsule; numerical evidence retains the actual intermediate wire. Inputs are delivered through the declared external Fore; the Pi input uses the existing mathematical Quantity wire.

| Delivered input value | Exact intermediate (mathematical notation) | Consuming numerical requirement |
| --- | --- | --- |
| `1m` | `100cm` | absolute error at most `0.01cm` |
| `1turn` | `2π rad` | absolute error at most `0.01rad` |
| Pi in radian coordinates | `1/2turn` | zero error; binary32 `0x3f000000` |
| `1rad` | `1/(2π)turn` | nonzero certified error within `0.001turn`; binary32 `0x3e22f983` |

The circle bridge uses the existing explicit `physical/angle-turn-radian-2pi@1` law. The prepared mathematical builder and enclosure evaluator are reused. Division by the named nonzero Pi constant is checked by both construction and decoding. Reciprocal interval evaluation requires an enclosure excluding zero; a coarse enclosure crossing zero returns `DivisorIntervalContainsZero` and charges attempted work.

Core tests also cover the source-declared milliradian scale, exact inverse conversion to degrees, rational fallback, affine Point/Delta distinctions, refusal atomicity and unchanged prepared capacity. Final instrumented Std tests execute the reciprocal-Pi chain through the kernel and independently replay both reports.

The four product executions retain eight output-Fore certificate values and four intermediate Cord values. Independent verification checks byte digests and exact producer, port, track, Plan, Play and Sign correlation. Twelve ordinary `conduit inspect` subprocesses reproduce the product-test inspection output, including semantic certificate replay and the original Plan request check. These checks do not authenticate an external execution actor.

Validation retained in the receipt:

- Full Core: 596 passed, zero failures or ignored tests; all 111 executed test binaries preserved.
- Full Std library at the production checkpoint: 886 passed, zero failures, 18 ignored. The final test-only addition is covered by eight passing coordinate tests.
- Focused product: four executions; the existing Fore export regression additionally passes across seven executions. Three inspection regressions pass.
- Strict Core/Std/product library, binary and test lint, Core native no-default-features compilation, formatting and patch checks pass.

Source bundles, raw exported artifacts, logs and verified executable archives remain in task custody. This evidence establishes the described local execution and replay scope. Full workspace, hardware and stable-main acceptance remain separate gates. N5's Source-derived qualified composite arithmetic, accumulated intermediate error and broader target requirements remain open.
