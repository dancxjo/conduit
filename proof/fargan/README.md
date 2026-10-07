# Development numerical proofs

`semantics/ai/tests/fargan_conditioning_graph.rs` runs the checked conditioning Source through ordinary Plan admission and the kernel scheduler. Numeric Backs execute resource embedding, affine, concatenation, activation and atomic history operations. ConduitOS generic expression owners execute the source period and record projections. This is hosted execution, not booted ConduitOS or audible synthesis.

The ordinary test uses independent synthetic tensors and f64 reference arithmetic. The explicitly ignored differential requires local pinned development data; CI never downloads weights. The model archive's explicit redistribution grant remains unresolved, so this directory contains no pretrained tensors.

The optional fixture uses xiph/opus commit `503d81b138d76621aae4b12786e90de48aa8db3a`, generated C digest `68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c`, and the exact F32 resource blob/digests checked by the test. Its manifest indexes generated arrays by logical dimensions and explicit packing. Feature fixture: 24 frames of 20 IEEE F32 little-endian values; features are zero except feature0 (-8 for frames0..7 and16..23; -2 otherwise), feature18 (`log2(256 / period) - 1.5`, period80 for frames0..11 and160 thereafter), feature19 (0 or0.8 on the same energy intervals). This tests network behavior, not pronunciation or normalization compatibility.

Build the scalar full-float oracle with `build_conditioning_oracle.sh PINNED_OPUS_SOURCE DEVELOPMENT_FIXTURE_DIRECTORY`. The source directory must contain the pinned generated data and upstream dependencies. Then explicitly run:

```sh
CONDUIT_FARGAN_DEVELOPMENT_FIXTURE=/absolute/path/to/development-fixture \
  CARGO_INCREMENTAL=0 cargo +stable test -p conduit-ai --features kernel-step \
  --test fargan_conditioning_graph pinned_scalar_model_conditioning_differential \
  -- --ignored --nocapture
```

The fixture contains `resources/f32.bin`, `resources/manifest.json`, `conditions.f32le`, and `conditioning.trace`. Full-float scalar trace digest is `ca634e3e31af975329ed3db41d38cf1d8c05a164ed0d8b16bcebcd270e5faacd`; the test refuses a stale or compact-profile trace. `DISABLE_DEBUG_FLOAT` removes float matrices and selects compact weights in several layers; that build requires separate comparison evidence.

Observed 24-frame full-float maximum absolute error: conditioning0.00027418137, retained history0.00005865097. The selected generic activation uses libm; upstream uses a scalar rational approximation, so this is numerical agreement within explicit test thresholds, not bit parity. The driver carries source-produced history between frames, publishes delayed/current pitch separately, and begins with zero conditioning history and previous period69. Full signal/recurrent state, PCM, compact network parity, ModelArtifact/session correlation and physical listening remain outside this conditioning proof.

## Compact matrix operators

`build_compact_linear_oracle.sh PINNED_OPUS_SOURCE DEVELOPMENT_FIXTURE_DIRECTORY` builds the scalar signed-Q7 profile with compact weights, SIMD/FMA disabled, and no unsigned-input bias substitution. It writes 48 unactivated matrix results: zero, bounded varying and alternating endpoint inputs for each of 16 matrices. The oracle is development evidence only; its record list does not drive the product network.

The optional `fixed_compact` test verifies the pinned compact blob, every consumed array digest and the complete trace digest `a7cc9ddf71bdb661141dab60929e2e0ce716f9490c04d7e66f0238a5ad00480e`. Run it with the same explicit fixture environment:

```sh
cargo +stable test -p conduit-ai --features kernel-step --test fixed_compact \
  pinned_compact_linear_differential -- --ignored --nocapture
```

Observed maximum absolute error across all 48 cases: **0**. Packed traversal uses input groups of four and output groups of eight, signed Q7 rounding, output scales and optional declared bias. GRU projections retain their genuinely absent bias rather than constructing a synthetic tensor. This proves the scalar compact matrix operations for these inputs; complete compact network state and PCM remain unproven.
