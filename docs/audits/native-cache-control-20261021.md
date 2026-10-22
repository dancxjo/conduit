# Matched native Source retention control

Prepared on the pending #6214 dependency (`0e6ffe777`); this is instrumentation
and a controlled measurement entrance, not a measured speedup or issue closure.

`cargo xtask make conduitos native-latency-proof --trials 3 --cache-mode retained`
and the same command with `--cache-mode zero-retention` each build one explicitly
instrumented architecture-proof image and run three fresh sequential QEMU
processes without retries. The ordinary product keeps its existing cache policy.
The zero-retention feature implies latency instrumentation and is selected only
by this observation entrance. No ordinary profile has a disabling option.

Both arms retain exactly the same installed catalogs. The control only disables
canonical checked Source and expanded Plot retention, using the existing
`SourcePreparationCache` zero-limit cold path. This comparison cannot attribute
total catalog-installation reuse versus pre-#6191 code. Preparation failure may exclude a trial; this accounting parser does not certify
arbitrary rejected-Source recovery. Current Host offer
planning, lowering, admission, kernel execution, and finite workload remain in
their existing paths. Historical measurements are context, not a matched arm.

Each cache record names the actual static Source digest, plot, mode, limits,
shared-store or private-contention path, preparation outcome, canonical stats
delta and that store's retained counts. Deltas are sampled before and after each
actual preparation. The consumer uses checked sums, including independent cold
fallbacks without resetting shared totals; private caches never claim shared
hits. Records are bounded to 512 per Boot and 768 bytes each; sequence, counter
saturation/reset or formatting exhaustion refuses the observation rather than
changing the operation's outcome. A fallback remains the existing finite cold
path and cannot spin, retry or block preparation.

Every cache call already owns exactly one PlotBuild timing interval; the parser
requires equal completed interval/snapshot counts, including the final call.
Mode and limits must match the guest. The parser verifies Source digests against
the exact tool source, rejects absent families, sequence gaps, reordered/stale/foreign records, false
hits and retained entries in the zero-retention arm, and correlates every Face
custody record to the observed Host/Boot. Early records are Boot-scoped just as
early progress records are; they precede identity generation. The family and
per-trial receipts bind clean Source, image hash, selected/observed mode,
profile/build and fresh Host/Boot identity. The existing eleven-field progress
schema, 512-byte/2048-record bounds and all required phases remain unchanged.
Hits emit no manufactured parsing/checking/expansion spans.

The first retained-arm attempt on `550247205` completed its functional journey
(24 receipt steps and 29 captured checkpoints), but the family failed the
unchanged required progress gate: `plot-build` and `scanout` were absent. Trials
1 and 2 were excluded and the zero-retention arm was not started. This is a
failed measurement, not accepted timing or a matched comparison. The preserved
packet remains unchanged. The correction brackets actual native preparation
and nonidle protected framebuffer copies plus acknowledgement; idle compositor
polls do not manufacture scanout phases. Existing parser, span and record bounds
remain unchanged. Corrected-source families are still pending.

Before any causal reduction claim,
retain both complete three-process families at the same clean Source and tools,
explicitly preserve the different mode/image identities, compare exact canonical
meaning and action/typed-result/refusal behavior using their original receipts,
and report boot and individual action attribution separately from harness wall
and capture overhead. Raw Plan/Face/Show identifiers legitimately vary with each
fresh Boot and must not be equated across Boots. Raw ticks remain uncalibrated,
nested and nonadditive; emulator measurements establish no physical latency.

## Corrected matched result (2026-10-22 UTC)

Both arms completed three fresh sequential QEMU trials on clean frozen Source
`0d53819e2faa129b49bd6e324894cec2bf232143`, with identical tools/profile and
104 accepted preparation requests per trial. Retained mode made five cold
checks/expansions and 99 hits; zero retention made 104 checks/expansions and no
hits, held entries or evictions. All 21 required phases passed in all six
trials. The independent comparison verified 174 original PNG byte/dimension/
decoded-RGBA identities and selected Source/graph/action/edit/reading outcomes.

| Observation | Retained median | Zero-retention median |
| --- | ---: | ---: |
| Full harness journey | 21.360 s | 56.822 s |
| Birth input command to validated active-Play capture upper bound | 3.300 s | 34.414 s |

The first interval includes process/input/poll/capture/teardown; the second
includes QMP/serial/poll/capture and is not exact guest input-to-Play latency.
These represent 62.4% and 90.4% lower observed medians respectively, within this
three-per-arm experiment. Arm order and host scheduling were not randomized;
compiler/observer overhead remains part of the instrumented appliance. Both
arms already share installed catalogs: this isolates retention, not all
historical catalog-installation reuse. Raw tick frequency remains unknown.

Strict raw metadata equality initially refused. Crèche emits its ready marker
before its new PRODUCT_JOURNEY row, so four snapshots retain the preceding
presenter field. All six original captured serial prefixes already contain the
same actual acknowledged component Show. The comparison checks that Show's
Host/Boot/Face/manifestation/mask custody only at this checkpoint and preserves
the raw discrepancy. Fresh Birth request IDs are checked against each exact
Boot before correspondence. Other presenter fields are unchanged. Five negative
controls refuse wrong Show Boot/presenter, absent pre-ready Show, wrong Birth
Boot and additional unexamined Accepted fields. Future producers should wait
for the complete post-ready journey record matching the acknowledged Face
before capture; these original families are not rewritten or retried.

[Permanent corrected packet and independent verifier](https://github.com/dancxjo/conduit/blob/f3f96770217592008929ca007544124fe9801e07/docs/audits/evidence/native-cache-control-20261022/corrected-0d53819e2/README.md)
retains all six families, original hashes, raw metadata refusal, comparison,
negative controls and exact image/build identities. The earlier failed `550`
packet is unchanged. Measured Source stays `0d53819e2`; the subsequent merge of
accepted dev `84dac6d02` produced `2eda544dc` with the exact same tree
`74a10c946a2b22a6dbda04871d3d593b8682caca`. This audit is a later documentation
successor, not a relabelled measured image. Candidate, merged Integration and
applicable publication acceptance remain pending.

### Attribution and no-gain boundaries

Boot-entry to first framebuffer commit medians were 7,543,036,800 retained versus
8,008,262,470 control raw ticks; boot-entry to actual first usable input ACK was
10,247,523,404 versus 10,912,109,252 ticks. These are approximately 5.8% and 6.1%
lower in this experiment, not seconds or the full-journey reduction. Median
aggregate nested check ticks were 1,208,877,386 versus 36,461,846,898, expansion
1,726,322,856 versus 61,514,272,822; both have five versus 104 actual cold scopes.
Catalog installation remains one per trial in both arms. Do not add nested
phase costs into exclusive CPU time.

Typing observation medians (three trials per arm), in milliseconds:

| Input row | ACK retained/control | Capture retained/control |
| --- | ---: | ---: |
| 1: a | 96.401 / 100.999 | 264.937 / 270.693 |
| 2: Backspace | 91.855 / 92.570 | 261.723 / 267.521 |
| 3: c | 100.273 / 101.481 | 273.298 / 277.474 |
| 4: a | 101.800 / 95.580 | 281.535 / 274.630 |
| 5: f | 100.788 / 97.338 | 282.160 / 282.025 |
| 6: AltGr+e | 192.421 / 192.636 | 374.674 / 375.443 |

The last input commits the Unicode editor result `café`. These are host-observed
command-to-ACK/capture intervals including polling, not physical keyboard or
scanout latency. Results are mixed: no consistent typing responsiveness gain
is established. Cache retention reduces repeated preparation; it does not
establish a general interaction, redraw or scheduling improvement.

The [pointer supplement](https://github.com/dancxjo/conduit/blob/e3eb6f53846384be19858fe12e6017690bdd08ac/docs/audits/evidence/native-cache-control-20261022/corrected-0d53819e2/pointer-supplement/README.md)
validates both exact left-button actions across all six trials through press
edges, accepted typed `creche.plot.3` toggles and new Available Face/Show
captures, including Host/Boot and serial-prefix custody. Four negative controls
refuse wrong Show Boot/action Show, a nonpress command and a missing action.
Command-to-capture upper-bound medians were 637.785/626.666 ms retained versus
659.862/680.698 ms control for select-three/restore-four. Deliberate held-input
waits, QMP/serial polling and screenshot work are included. These are not guest
input/ACK latency or isolated renderer time. Pointer-motion-to-redraw latency
cannot be established from these packets and remains an explicit responsiveness
follow-on; no general interactive gain is claimed by the cache comparison.
