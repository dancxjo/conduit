# Local GPU projection feasibility

**Defer a per-Gear GPU numeric provider for the measured workload.** Normal-input
matrix projections regress after upload, launch, synchronization and download.
A resident whole-frame island is a different experiment. No Conduit GPU Back,
Host offer, authority grant or published product capability is introduced;
conditional provider work remains open under #6105.

## Reproduce

From a clean committed checkout with the pinned Rust toolchain, local CUDA/nvcc,
`g++-13`, Python 3 and the explicitly supplied private pinned fixture:

```sh
cargo xtask prove runtime-performance --gpu-feasibility \
  --fargan-fixture /path/to/private/fixture \
  --output /path/to/new/capture --repetitions 9 --device 0 --quota-mib 1
```

The command never downloads or publishes model data. It verifies the pinned
upstream source, complete model blob and both exact tensor spans; observes the
actual device UUID, driver and compute features; compiles for that device; and
retains source/tree/worker hashes, compiler flags, the captured accepted-main ref,
raw trials, failures and cleanup. The capture directory must be new. Missing
resources, CUDA, a compiler or a device, and failed numeric checks are refusals
without CPU fallback. Private tensors remain outside Git.

## Observation and decision

The [supported-command record](../../proof/performance/gpu/supported-observation-20261020.json)
contains six cases, nine trials per case, and three refusal controls. Actual
pinned FARGAN matrices and controlled normal, tiny-normal and subnormal inputs
exercise the unbiased input-major ordered-F32 projection law in
`FixedTensorLinear`. Separate multiply/add rounding and disabled contraction/FTZ
produce bitwise-equal outputs in all 54 trials. This is a projection component
experiment, not complete dense/GRU/frame/Body execution.

| Normal-input projection | CPU median | GPU median including transfers |
|---|---:|---:|
| 272 → 480 | 7.17 µs | 15.75 µs |
| 128 → 40 | 0.48 µs | 12.58 µs |

Every first/cold trial is retained. Context, allocation and weight upload are
separate from warm trials. Weights remain resident for nine calls; input upload
and output download occur on each call. Exact output acceptance, synchronization,
cleanup and owner lifetime through host-buffer disposal are recorded. Process
wall, JSON inspection, compilation and pinned-resource validation are separate.
There is no calibrated machine timing or exclusive-device claim; CPU runs first.

Subnormal controls sometimes favor GPU execution because strict subnormal CPU
arithmetic is expensive. Those positive results are retained. They do not prove
a beneficial ordinary FARGAN distribution or a production eligibility rule.
Normal short-workload regressions and cold context costs defeat this per-Gear
proposal; they do not establish that every possible GPU island is unbeneficial.

## Finite ownership and limits

The experiment admits one process-owned context and three explicit device
buffers. Exact requested payload bytes must fit the selected quota and observed
available memory before allocation: 525,248 bytes for the large projection and
21,152 for the short projection. Every CUDA operation is checked. Allocation,
transfer, launch, synchronization and cleanup failures are terminal. Invalid
device, no-visible-device and insufficient-quota controls refuse without fallback.
Success releases all three buffers and the owned context.

The payload quota excludes opaque CUDA context/driver memory. It does not provide
hostile-code confinement, cross-process reservations or Conduit GPU resource
admission. That gap is another reason not to advertise a provider. Context
creation failure is checked, but no actual driver/device-loss event was induced.
These are local observations, not a calibrated product speedup claim.

The [initial hardware family](../../proof/performance/gpu/initial-observation-20261020.json)
and the preceding [GCC AMX-header compilation failure](../../proof/performance/gpu/initial-compiler-failure-20261020.txt) are preserved separately;
no successful retry overwrites a failed trial. The supported family owns its
committed source. Stable publication remains under #6078; development experiment
closure does not assert a released GPU capability.

Altered upstream-source and named-tensor metadata also [refuse before device probing or compilation](../../proof/performance/gpu/custody-controls-20261020.json).
