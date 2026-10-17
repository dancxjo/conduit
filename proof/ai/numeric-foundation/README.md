# Finite Source arithmetic and bounded affine reference foundation

This extracts reusable numerical capability from the preserved FARGAN draft.
It does not complete or close #4898 or #4907. No trained model, Speech topology,
ConduitOS guest, Std discovery factory, streaming neural graph or audio quality
claim is included.

The Source capability accepts contextual exact IEEE F32 bit literals, finite
arithmetic and ordering. Equality retains the existing encoded-value identity
relation, including distinct positive and negative zero. Original nominal
collection construction and nominal scalar framing fixes are required for
reference/prepared parity; their genuine prerequisite failures are retained.
No refinement guard or Type bound was weakened.

The AI capability provides scalar fixed-shape reference kernels, exact tensor
content/shape admission, an ordinary prepared affine Plan Back, and immutable
owned resources including no-copy shared slices. The actual owned affine
scheduler journey passes pressure, cancellation and resource identity tests.
It retains the established borrowed and owned APIs from the original work.
Reference kernels are scalar, not a performance or whole-runtime budget claim.
Parent-model admission and tensor-slice correlation remain caller obligations.

## Validation boundary

AI all-target tests pass: 279 tests across 74 executables. Plot all-target tests
pass: 446 tests across 29 executables. Their source is represented by
`26a26d5a407f934e6cb2dda879896cbad70c1e3e`; the Plot run started before the
AI-only fixture edits, with unchanged Plot sources throughout. The final source
`dc58979826913e686c0ccdc74bdee8cbbf23e802` changes only module declaration order
in two facades. Strict combined all-target Clippy and workspace formatting pass
on that final source. This evidence commit changes only proof files.

Commands used cargo +stable, two jobs, zero dev debug information, no incremental
compilation, and the existing shared target directory. Final gates:

```sh
cargo +stable test --locked -p conduit-plot --all-targets
cargo +stable test --locked -p conduit-ai --features kernel-step --all-targets
cargo +stable clippy --locked -p conduit-ai -p conduit-plot --features conduit-ai/kernel-step --all-targets -- -D warnings
cargo +stable fmt --all --check
```

`manifest.json` records exact source identities and byte extents/SHA-256 for
byte-exact producer logs. Empty formatting output is a successful quiet check.
The original full FARGAN head is `f813938ba22cb23e36181f3e1b9ee2e3c3edb61c`;
its remaining implementation and evidence are not replaced by this foundation.
