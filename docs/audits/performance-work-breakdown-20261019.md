# Performance audit: ranked work and origin handoff

Tracker: [#6078](https://github.com/dancxjo/conduit/issues/6078). This index covers the complete supplied audit, including held implementations, missing engineering, measurement and release acceptance.

**Rank means estimated remaining effort and risk, easiest to hardest. It is not priority, expected speedup or execution order. Dependencies below govern execution.** Existing product acceptance criteria stay authoritative; no issue is closed by this inventory.

## Ranked issues

| Rank | Issue and scope | Prerequisites in this plan |
|---:|---|---|
| 1 | [#6079 — Enable immutable numeric catalogue reuse in the ordinary Std Host](https://github.com/dancxjo/conduit/issues/6079) | Check current base and ownership |
| 2 | [#6080 — Verify published prepared-session foundation against accepted main](https://github.com/dancxjo/conduit/issues/6080) | Check current base and ownership |
| 3 | [#6081 — Finish integrated release acceptance for borrowed two-host Line codecs](https://github.com/dancxjo/conduit/issues/6081) | Check current base and ownership |
| 4 | [#6082 — Integrate and release compact F32 DAGs and local prepared expressions](https://github.com/dancxjo/conduit/issues/6082) | Check current base and ownership |
| 5 | [#6083 — Integrate borrowed Body Line receive, binding and outbound offer paths](https://github.com/dancxjo/conduit/issues/6083) | [#6081](https://github.com/dancxjo/conduit/issues/6081) |
| 6 | [#6084 — Measure and optimize release-profile build generators](https://github.com/dancxjo/conduit/issues/6084) | Check current base and ownership |
| 7 | [#6085 — Narrow generated-artifact rebuild invalidation with exact dependency evidence](https://github.com/dancxjo/conduit/issues/6085) | [#6084](https://github.com/dancxjo/conduit/issues/6084) |
| 8 | [#6086 — Defer descriptive Sign reconstruction on ordinary Host inspection paths](https://github.com/dancxjo/conduit/issues/6086) | [#6082](https://github.com/dancxjo/conduit/issues/6082) |
| 9 | [#6087 — Integrate bounded exact-source checking and expansion reuse](https://github.com/dancxjo/conduit/issues/6087) | [#6082](https://github.com/dancxjo/conduit/issues/6082), [#6088](https://github.com/dancxjo/conduit/issues/6088) |
| 10 | [#6088 — Integrate shape-selected input-major Linear and Slice numeric lowering](https://github.com/dancxjo/conduit/issues/6088) | [#6082](https://github.com/dancxjo/conduit/issues/6082) |
| 11 | [#6089 — Integrate revision-safe retained raster reuse in ConduitOS](https://github.com/dancxjo/conduit/issues/6089) | Check current base and ownership |
| 12 | [#5546 — Benchmark fusion cost and report target-specific unsupported limits](https://github.com/dancxjo/conduit/issues/5546) | [#6082](https://github.com/dancxjo/conduit/issues/6082) |
| 13 | [#5761 — Measure native FARGAN resource cost and real-time factor](https://github.com/dancxjo/conduit/issues/5761) | [#6082](https://github.com/dancxjo/conduit/issues/6082), [#6087](https://github.com/dancxjo/conduit/issues/6087) |
| 14 | [#6090 — Measure ConduitOS boot, first usable input and interaction stages](https://github.com/dancxjo/conduit/issues/6090) | [#6089](https://github.com/dancxjo/conduit/issues/6089) |
| 15 | [#6091 — Measure and reduce copies on established warm cross-host Lines](https://github.com/dancxjo/conduit/issues/6091) | [#6081](https://github.com/dancxjo/conduit/issues/6081), [#6083](https://github.com/dancxjo/conduit/issues/6083) |
| 16 | [#6092 — Publish a matched five-workload cold and warm performance baseline](https://github.com/dancxjo/conduit/issues/6092) | [#5546](https://github.com/dancxjo/conduit/issues/5546), [#6016](https://github.com/dancxjo/conduit/issues/6016), [#5761](https://github.com/dancxjo/conduit/issues/5761), [#6090](https://github.com/dancxjo/conduit/issues/6090), [#6091](https://github.com/dancxjo/conduit/issues/6091) |
| 17 | [#6093 — Load optional browser adapters on demand with exact bundle and activation custody](https://github.com/dancxjo/conduit/issues/6093) | Check current base and ownership |
| 18 | [#6094 — Invalidate only the smallest provably affected checked and expanded subgraphs](https://github.com/dancxjo/conduit/issues/6094) | [#6087](https://github.com/dancxjo/conduit/issues/6087) |
| 19 | [#6095 — Reuse immutable prepared Back fragments and type specializations with exact keys](https://github.com/dancxjo/conduit/issues/6095) | [#6094](https://github.com/dancxjo/conduit/issues/6094), [#6082](https://github.com/dancxjo/conduit/issues/6082) |
| 20 | [#6096 — Adopt reusable prepared Value sessions in one ordinary Body lifecycle](https://github.com/dancxjo/conduit/issues/6096) | [#6080](https://github.com/dancxjo/conduit/issues/6080), [#6082](https://github.com/dancxjo/conduit/issues/6082) |
| 21 | [#6097 — Reuse prepared numeric and Source resources through an ordinary Fore](https://github.com/dancxjo/conduit/issues/6097) | [#6096](https://github.com/dancxjo/conduit/issues/6096), [#6095](https://github.com/dancxjo/conduit/issues/6095), [#6088](https://github.com/dancxjo/conduit/issues/6088) |
| 22 | [#6098 — Measure truthful GPU admission, transfer costs and numeric feasibility](https://github.com/dancxjo/conduit/issues/6098) | Check current base and ownership |
| 23 | [#6016 — Specialize compact Quantity regions without per-operation canonical materialization](https://github.com/dancxjo/conduit/issues/6016) | [#6082](https://github.com/dancxjo/conduit/issues/6082) |
| 24 | [#5545 — Retain per-Gear observability and exact failure attribution under fusion](https://github.com/dancxjo/conduit/issues/5545) | [#6082](https://github.com/dancxjo/conduit/issues/6082) |
| 25 | [#6099 — Admit typed numeric scratch and atomic output ownership without publication copies](https://github.com/dancxjo/conduit/issues/6099) | [#6082](https://github.com/dancxjo/conduit/issues/6082), [#5545](https://github.com/dancxjo/conduit/issues/5545) |
| 26 | [#6100 — Compile eligible numerical islands into bounded typed instruction programs](https://github.com/dancxjo/conduit/issues/6100) | [#6099](https://github.com/dancxjo/conduit/issues/6099), [#6088](https://github.com/dancxjo/conduit/issues/6088) |
| 27 | [#6101 — Expose measured preparation, transfer and resident-resource costs for replanning](https://github.com/dancxjo/conduit/issues/6101) | Check current base and ownership |
| 28 | [#5568 — Admit hysteresis and net-benefit replan policy with fresh costs](https://github.com/dancxjo/conduit/issues/5568) | [#6101](https://github.com/dancxjo/conduit/issues/6101) |
| 29 | [#5571 — Prove incremental cache validity against cold planner oracle](https://github.com/dancxjo/conduit/issues/5571) | Check current base and ownership |
| 30 | [#6102 — Preserve old Play outcomes and resident resources across Plan replacement](https://github.com/dancxjo/conduit/issues/6102) | [#5571](https://github.com/dancxjo/conduit/issues/5571), [#6101](https://github.com/dancxjo/conduit/issues/6101) |
| 31 | [#5563 — Plan immutable region partitions and admitted finite lane resources](https://github.com/dancxjo/conduit/issues/5563) | Check current base and ownership |
| 32 | [#6103 — Offer a measured SIMD numeric Back with exact semantic eligibility](https://github.com/dancxjo/conduit/issues/6103) | [#6088](https://github.com/dancxjo/conduit/issues/6088), [#6100](https://github.com/dancxjo/conduit/issues/6100) |
| 33 | [#6104 — Integrate bounded-response deadline admission with global placement](https://github.com/dancxjo/conduit/issues/6104) | [#6101](https://github.com/dancxjo/conduit/issues/6101) |
| 34 | [#5564 — Execute two independent admitted pure regions on hosted workers](https://github.com/dancxjo/conduit/issues/5564) | [#5563](https://github.com/dancxjo/conduit/issues/5563) |
| 35 | [#5566 — Publish physical-lane equivalence and resource evidence](https://github.com/dancxjo/conduit/issues/5566) | [#5564](https://github.com/dancxjo/conduit/issues/5564) |
| 36 | [#5565 — Prove fair responsive interactive work beside batch saturation](https://github.com/dancxjo/conduit/issues/5565) | [#5564](https://github.com/dancxjo/conduit/issues/5564), [#5566](https://github.com/dancxjo/conduit/issues/5566), [#6104](https://github.com/dancxjo/conduit/issues/6104) |
| 37 | [#6105 — Implement one admitted GPU numeric Back only after a beneficial feasibility result](https://github.com/dancxjo/conduit/issues/6105) | [#6098](https://github.com/dancxjo/conduit/issues/6098), [#6100](https://github.com/dancxjo/conduit/issues/6100), [#6101](https://github.com/dancxjo/conduit/issues/6101) |

## Source already preserved on origin

| Work | Origin branch / PR | Retained proof revision |
|---|---|---|
| [#6079](https://github.com/dancxjo/conduit/issues/6079) | [`perf/std-catalog-cache-current`](https://github.com/dancxjo/conduit/tree/perf/std-catalog-cache-current) | [`13e977a0277a`](https://github.com/dancxjo/conduit/commit/13e977a0277afdc135550673924c90812248ce44) |
| [#6082](https://github.com/dancxjo/conduit/issues/6082) | [`perf/compact-f32-current`](https://github.com/dancxjo/conduit/tree/perf/compact-f32-current) | [`1048d498f2fc`](https://github.com/dancxjo/conduit/commit/1048d498f2fc872f9f8175b834cf327d4cad96eb) |
| [#6083](https://github.com/dancxjo/conduit/issues/6083) | [`perf/body-line-borrowed-receive`](https://github.com/dancxjo/conduit/tree/perf/body-line-borrowed-receive) | [`d9e0d9018a7d`](https://github.com/dancxjo/conduit/commit/d9e0d9018a7d43aabe9045c308fa8a22b645b625) |
| [#6087](https://github.com/dancxjo/conduit/issues/6087) | [`perf/input-major-linear-36c`](https://github.com/dancxjo/conduit/tree/perf/input-major-linear-36c) | [`6ad1442dbfad`](https://github.com/dancxjo/conduit/commit/6ad1442dbfad482521758dfdec62f0503b959aac) |
| [#6088](https://github.com/dancxjo/conduit/issues/6088) | [`perf/input-major-linear-36c`](https://github.com/dancxjo/conduit/tree/perf/input-major-linear-36c) | [`6ad1442dbfad`](https://github.com/dancxjo/conduit/commit/6ad1442dbfad482521758dfdec62f0503b959aac) |
| [#6089](https://github.com/dancxjo/conduit/issues/6089) | [`perf/retained-raster-current`](https://github.com/dancxjo/conduit/tree/perf/retained-raster-current) | [`1379ae0cb657`](https://github.com/dancxjo/conduit/commit/1379ae0cb6574b7e27897d823d9e5228e035af5a) |
| [#5761](https://github.com/dancxjo/conduit/issues/5761) | [`perf/input-major-linear-36c`](https://github.com/dancxjo/conduit/tree/perf/input-major-linear-36c) | [`6ad1442dbfad`](https://github.com/dancxjo/conduit/commit/6ad1442dbfad482521758dfdec62f0503b959aac) |
| Prepared-session foundation | [PR #6045](https://github.com/dancxjo/conduit/pull/6045), merged | Published source 9ff; fresh independent product custody remains #6080 |
| Borrowed two-host codecs | [PR #6048](https://github.com/dancxjo/conduit/pull/6048), merged | Merge 1d1eca48d147; combined integration/release remains #6081 |
| Compact DAG/local expressions | [PR #6053](https://github.com/dancxjo/conduit/pull/6053), ready at inventory | Proof 1048; current origin head 241b merges newer dev and needs exact coverage |

## Current proof and limits

- Fresh source 1048 full FARGAN conformance and independent replay passed 48 paired frames,625,152 raw events, exact final outputs and per-node ordered transactions; all 96 Play invocations requested zero allocations. The canonical Candidate passed. This is not accepted quiet timing, compiled numeric code, latest combined-source proof or stable release acceptance.
- Held Body Line source d9e0 passed 3 release package tests, three matched roundtrips with 2 fewer allocation requests each, stale Boot behavior, a compiled outbound-admission omission refusal and scoped release Clippy. Full module-extraction workspace/integration gates remain.
- Held Source-cache source 6ad passed independent 144 paired-frame replay/1,864,656 events with 288 allocation-free Plays. The whole timing family was excluded for foreign CPU activity; no subset was accepted. Family 3 retains its original registration and zero-process busy preflight.
- Retained raster QEMU conformance exists on held source 1379; its timing family was excluded for compiler activity. Actual startup/usable-input/interaction stages are a separate ticket.
- Actual GPU enumeration identified RTX 3060 / compute 8.6 / 12,489,654,272 B VRAM and nvcc. Device inventory is not context, allocation, transfer, kernel, admission or speedup proof.
- The latest release observed during this inventory is [release-9ff334927f546c7f9a4a241ef38b548bd1b25549](https://github.com/dancxjo/conduit/releases/tag/release-9ff334927f546c7f9a4a241ef38b548bd1b25549). The older full product-archive verifier report in this bundle is explicitly for source a258, not this newer release.

## Existing work and issue ownership

- Preserve released Flow ownership PR #6019, preparation/catalogue PR #6021 and span compositor/scanout PR #6024. Preserve existing compact Sign records and the browser size regression fix. The remaining tickets extend or verify them rather than starting over.
- #5715 owns ordinary streaming FARGAN. Foundation fixtures and a numeric measurement profile do not establish full product speech or resource acceptance.
- #5230 owns global placement; its completed bounded search is not rewritten. #5233 owns reluctant replacement; #5567/H1 is already closed. H2/H3 and old-Play custody remain separate work.
- #5232 and #5563–#5566 own actual physical CPU lanes. Plan lane metadata alone is not physical overlap.
- #6016 retains its exact Quantity laws and QC2/QC3 prerequisites. #5545/#5546 retain original fusion inspection/measurement requirements. Closed #5396/#5397 are ancestry and are not reopened.

## Audit coverage

| Supplied audit requirement | Owning work |
|---|---|
| Five-workload cold/warm baseline, allocations, bytes, scheduler work and Signs | #6092; #5546; #5761; #6016; #6090; #6091 |
| Reusable prepared graph/weight/code/binding/reservation lifetime | #6080; #6095; #6096; #6097; existing #5715 |
| True numeric islands, scratch and avoided canonical materialization | #6082; #6088; #6099; #6100; #6016 |
| Hosted Flow ownership and Cross-host Line copying/framing | PR #6019; #6081; #6083; #6091 |
| Per-pixel span/scanout and retained incremental raster | PR #6024; #6089 |
| Boot/paint/usable input/redraw/pointer/typing/Plot-stage responsiveness | #6090 |
| Immutable checking/expansion/type/law/prepared Back reuse and minimal invalidation | #6079; #6087; #6094; #6095 |
| Compact Signs, interned identities and deferred reconstruction | Existing numeric records; #6086; #5545 |
| Bounded greedy/global placement and reluctant exact replanning | #5230; #6104; #6101; #5568; #5571; #6102 |
| Real multicore overlap, deterministic commit, fairness | #5563; #5564; #5566; #5565 |
| Justified SIMD and conditional GPU resource/transfer/numeric proof | #6103; #6098; #6105 |
| Browser demand loading and initial footprint | #6093 |
| Release generator optimization and exact rebuild invalidation | #6084; #6085 |
| Actual dev/main automatic release and retained-product/source custody | #6080; #6081; #6082 and every implementation ticket’s closure criteria |

## Evidence bundle and custody

The adjacent [evidence directory](evidence/performance-handoff-20261019/) contains the supplied audit, exact-copy selected validation records, unchanged verifiers/controllers, the machine-readable issue plan and observed origin routing. [retained-files.json](evidence/performance-handoff-20261019/retained-files.json) records copied-file hashes and scopes. Read each record’s source and limitations before using it.

Private models/fixture inputs, large raw local captures, worker binaries and build caches are not committed. Their retained hashes and summary records are not a claim that those artifacts are public or that a newer source/release has been verified. Verifiers require the corresponding authorized raw inputs. No timing thresholds or existing registrations were weakened by this handoff.

## Execution and closure

Refresh the smallest owned delta onto current dev when its dependencies and review slot are ready. Keep at most two overlapping implementation PRs in review and at most one merge-ready shared-core PR. Preserve held branches and do not absorb other owners’ unmerged changes. Start with the low-effort catalogue activation and already implemented release/integration tickets; start deeper provider work only after its explicit prerequisites.

Every change preserves exact Source/Plan/Host/Boot/Back identity, typed ports, finite pre-Play resources, owner/fuel/pressure/cancel checkpoints, separate ownership/ACK/delivery and mandatory evidence. Stable main receives only the supported automatic release train after exact-source full dev Integration, with retained tested artifacts and identical accepted tree. No parent closes until its actual product and release requirements are proved.
