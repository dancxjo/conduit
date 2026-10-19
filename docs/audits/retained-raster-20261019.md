# Retained native raster reuse and observed journey latency

Development implementation and conformance evidence, 2026-10-19. Stable release
acceptance and accepted idle latency measurements remain open.

The native surface update previously cleared and replayed the entire retained
scene for every local edit. The new path retains exact unaffected pixels and
replays only commands intersecting admitted damage. Full invalidation continues
through the original full renderer. This builds on the separately merged span
compositor and word scanout work; their effects are not attributed to this patch.

Reuse uses the same admitted surface buffer and visual expression. Existing
Plan, placement, front-subject, manifestation and Face revision fences remain
authoritative. Only canonically decoded scenes enter the retained path;
noncanonical input keeps the original renderer and its refusals. Changed command
footprints expand bounded replay damage; full invalidation takes the full path. Reuse does not invent application state, change Face input eligibility,
or acknowledge a stale Show. It does not allocate in the measured prepared
raster path. Differential cases cover text, clipped orthogonal paths, cursor,
geometry, damage, revision and refusal behavior.

## Reproduce through supported entrances

```sh
cargo xtask make conduitos compositor-performance
cargo xtask make conduitos graphical-profile-proof
```

The first command runs optimized reference/retained memory-target comparisons,
span composition and raw scanout conformance. The second builds and boots the
canonical graphical x86 live image, drives ordinary Crèche/Body/Face actions,
retains screenshots and checks that four additional architecture images omit
graphical assets. QEMU and the pinned image-building tools are prerequisites.
These commands have different proof scopes; a memory-target ratio is not a
user-visible latency result.

After the canonical image is built, the existing supported replay entrance is:

```sh
cargo xtask make host verify target/conduitos/live/x86_64-pc --journey
```

It verifies the retained product image and resolved BUILD closure, requires the
image source revision to match the proof driver, then runs the journey without
rebuilding the guest. Preserve each completed capture before another replay,
because the journey artifact directory is reused. Image hashes are checked
before and after the journey. This is the entrance for fresh-process repetitions
of one fixed image; it does not by itself establish an idle measurement role.

## Retained raster measurements

The retained worker at source `65d06e007` ran in three fresh processes pinned to
CPU 4, without this work's concurrent compilation. Each process passed nine
raster tests and retained 25 timing rows: five cases, five repetitions per case,
64 frames per repetition. The combined 75 raw rows and all three logs are in
[evidence](evidence/retained-raster-20261019/fallback-isolated.json).

| Case | Median retained/reference batch ratio |
| --- | ---: |
| Full redraw | 1.0320 |
| Tiny damage | 0.02461 |
| Scroll | 0.78795 |
| Hover | 0.04119 |
| Typing fixture | 0.03475 |

Full redraw shows a small overhead and no reduced pixel/command work. The local
cases show reduced replay work in these memory fixtures. These are descriptive
paired batches, without confidence intervals or a universal speedup claim.
The typing fixture is synthetic raster work, distinct from ordinary editor input.
Earlier differential proof covered 576 scene pairs; the current-source bridge
identifies the unchanged product files after refresh. Combined-base validation
is recorded separately and is not inferred from that bridge.

## Real graphical journey conformance

Source `48f22c2a9d58242a7538403561bcbabf6af74260` completed the full graphical
profile proof. Its x86 ISO SHA-256 is
`7ff8f131097fe0f799a6e00bc4a95aef1dbfe93c8ac83e194310891a839333f0`.
The receipt binds the four additional image hashes. The ordinary journey
completed 22 checkpoints, including keyboard selection, native editor input,
Unicode, two pointer gestures, lifecycle controls and Patchbay views.

The retained capture contains the image, build manifest, raw serial transcript,
QMP commands/responses, 22 PNGs and host observations. A portable independent
verifier decoded all 22 PNGs and matched their RGBA pixel digests. It checked 199
complete serial records, zero incomplete tail bytes, and reconstructed both
pointer feedback rows. Eight semantic corruptions were refused after refreshing
internal hashes. This proves the captured journey and evidence correlation;
it is not a before/after QEMU pixel-equality comparison.

The run was declared conformance-only before guest execution because other
validation and compilation were active. None of its durations count as accepted
idle measurements. The immutable archive and member inventory are retained in
this evidence directory. Its SHA-256 is
`5da68e99c8148f43d8f115db8ace53c66419d3d16178b6b28d644cdb9b6b5cdb`.

## Host-observed latency implementation

The proof driver timestamps QMP command/response observations, serial reads and
framebuffer captures on one process-monotonic nanosecond clock. The bounded
serial observer records complete raw line byte ranges, SHA-256, read intervals
and final drain, with explicit limits on raw bytes, records and output size.
Failures remain failed rather than silently producing an apparently healthy
partial report.

The report correlates pointer presses with exact current Face/Show identities
and fresh rendered changes. It also correlates six serialized ordinary editor
edits (`a`, erasure, `c`, `ca`, `caf`, `café`) with the active Plan, Play, checked
Plot and partition, new revisions, manifestation identities and input/result
Signs. The QMP acknowledgement belongs to the current request lifetime; it is
separate from guest consumption. Correlation uses exact raw receipts and ordered
clock observations, not screenshot delays or assumed input delivery.

The typing extension at `0f5aa42ed` was replayed against a relocated copy of the
immutable capture. A separate Python implementation reproduced all six rows and
rechecked all 22 framebuffer digests. Five additional corruptions were refused:
forged duration, forged manifestation, foreign Play, wrong physical key and
failed QMP acknowledgement. The original archive remained unchanged, and scratch
bytes were restored and reverified. The reconstruction source and original guest
capture source are recorded separately.

These are host-observed upper bounds including guest work, serial output,
host scheduling and observer delay. A rendered journey receipt follows native
refresh; it does not establish physical display scanout or exact first-paint
time. Six sequential editor edits do not establish sustained typing throughput.
Missing measurements remain explicit: guest boot-entry clock, sustained typing,
and separate build/check/seal/prepare/Play costs.

## Validation and remaining acceptance

The retained evidence records focused raster conformance, the supported optimized
memory command, package Clippy, full xtask Clippy, five focused latency tests,
the full graphical profile gate and independent replay/mutation results. The
new base includes the native numeric region and browser clock-proof repair;
[current source bridge](evidence/retained-raster-20261019/current-source-bridge.json)
records byte identity for all 15 graphics delta files. On that base, all eight
normal raster differential/refusal tests and full xtask all-target Clippy passed;
the optional timing fixture remained skipped. The first compile refused a
tracked snapshot fixture omitted by sparse checkout. Restoring that exact
tracked file resolved the infrastructure omission before the successful run.

Fresh idle QEMU runs, a source-bound responsiveness comparison, combined `dev`
integration, automated release promotion, exact `main` tree and published asset
verification remain required. No stable release or physical-device acceptance
is claimed here.
