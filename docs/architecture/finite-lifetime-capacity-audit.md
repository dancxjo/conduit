# Finite-lifetime capacity audit

Issue [#3638](https://github.com/dancxjo/conduit/issues/3638) owns this
repository-wide audit. The ledger distinguishes finite simultaneous embodiment
from finite lifetime throughput. It is intentionally evidence-oriented: an
entry is not `corrected` until a stress crosses the old bound without changing
the owning Body, Plan, Play, session, or admitted storage.

The classifications are:

- **A** reusable simultaneous capacity;
- **B** real concurrent or in-flight capacity;
- **C** authored semantic or transaction cardinality;
- **D** retained history/evidence capacity;
- **E** numeric-domain bound;
- **F** proof-fixture cardinality.

## Candidate ledger

| Location | Bound and owner | Class | Lifetime behavior and required policy | Owner / proof | Status |
| --- | --- | --- | --- | --- | --- |
| Browser Body input router | Eight pending inputs per Form and kind | A/B | Retire consumed ordered transitions; coalesce the newest pointer state in one slot; pressure only the affected stream. | #3636; 100,000 pointer observations and 1,024 ordered transitions through production DOM routing | corrected locally |
| Browser human-input adapter | Eight keyboard/button transitions; one pointer state | A/B | Keyboard and buttons remain ordered; pointer state coalesces; one stream's pressure must not terminalize the adapter or Body. | #3636; pinned Chromium production adapter stress | corrected locally |
| ConduitOS USB keyboard | 63 data TRBs plus one Link TRB; eight report buffers | A/E | Completed TRBs and report buffers recycle with exact Link/cycle semantics; only numeric sequence overflow remains eventual. | #3206; repeated wrap tests on current `dev` | corrected |
| ConduitOS USB pointer | 63 data TRBs plus one Link TRB; eight report buffers | A/E | Same physical-ring law as keyboard; portable observation sequence remains independent. | #3637; 1,024 lower-level reports and 1,050 real QEMU reports in one session | corrected locally |
| ConduitOS PS/2 pointer | One pending normalized sample | A | Coalesce newest absolute/button state and accumulated motion; retain explicit coalesced/drop evidence; preserve the old sample transactionally on evidence overflow. | #3637; 100,000 samples in one slot | corrected locally |
| ConduitOS PS/2 and portable keyboard queues | Fixed ordered transition queues | B | Refusal while genuinely occupied is correct; consuming a transition releases its slot. No overwrite or pointer-style coalescing. | Existing queue-pressure and ordering tests | classified correct |
| ConduitOS FTDI bulk rings | 128 TRBs per direction, formerly consumed once | A/E | Reserve one Link TRB, recycle 127 completed data slots independently per direction, and keep exact completion validation. | #3643; 100 ring laps below QEMU plus 130 ordered values and 260 acknowledgements in one real QEMU Line session | corrected locally |
| QEMU proof transport | 1,024 synchronous QMP request identities | A | A matching response retires the sole in-flight identity; event and message bounds remain independent. | #3638; 100,000 request-identity allocations and unchanged 1,050-report journey | corrected locally |
| Browser replay control | 64 queued values, formerly 128 lifetime Host requests | A/B | The one in-flight request retires before two fixed processing/event identities are reused. | #3644; 100,000 interactions and non-current-completion rejection | corrected locally |
| Browser Form runner pending effects | Preallocated pending effect vector | B | Slots are removed on exact completion; refusal applies only while every admitted concurrent slot is occupied. | `session_effects.rs` completion/removal and capacity tests | classified correct |
| Browser Host completion evidence | 64 recent outcomes plus omitted count | D/E | Ring replacement is the declared recent-history policy; `u64` sequence exhaustion remains distinct. | `host_outcomes.rs` wrap/gap test | classified correct |
| Generated-text flow | Maximum chunks, per-chunk bytes, and total output bytes | C/E | These bound one generated-output transaction and its value, not reusable runtime storage. A later generation creates a new semantic transaction. | `streaming_generation.rs` conformance tests | classified correct |
| Human and presentation interaction queues | Fixed pending transitions/events | B | Pop/ack releases ordered storage; genuine producer-over-consumer pressure refuses without overwrite. | semantic queue tests and browser presenter proof | classified correct |
| Kernel state-delay transition limit | Planned maximum transitions for one bounded operation | C | The limit is an admitted operation work budget and must remain visible as `WorkBudgetExhausted`; it is not a generic standing source lifetime. | kernel continuation/failure tests | classified correct |
| Kernel Cord/value queues | Planned item and byte capacity | B | Dequeue/ack retires occupancy; atomic fan-out must still refuse genuine pressure without partial delivery. | kernel queue, fan-out, and long-session tests | classified correct |
| Bounded Line transcripts | Fixed retained item/byte history plus gap | D | Old retention may be superseded only with explicit gap evidence; active Line delivery capacity is separate. | multihost transcript evidence and retention-gap tests | classified correct |
| Browser WebRTC sessions | Four current/creating sessions and four pending signals | B | These are simultaneous bounds; closed sessions release their slots. | #3609 / #3619 session lifecycle proof | classified correct |
| Browser WebRTC grant generations and retired negotiation set | `u32` generation plus one retirement-batch count | A/D/E | Signals carry their exact generation, stale generations are rejected, and retirement retains only a bounded count rather than identities. The native provider still requires a replacement Plan before it can issue a nonzero generation. | #3609; 100,000 browser replans, stale-generation rejection, and pinned-browser session proof | corrected locally / broader owner open |
| Timers and interval sources | Concurrent arms plus typed tick/value domains | B/E | Completion releases an arm; checked tick/value overflow remains real. Proof fixture tick counts do not constrain production lifetime. | timer nucleus and standing timer tests | classified correct |
| Audio synthesis/playback buffers | Fixed frames, voices, channels, and interval output | B/C | Frames are reused per interval; voice/channel saturation is simultaneous; authored render lengths remain transaction bounds. | synth/OPL2/PC-speaker stress and cancellation tests | classified correct |
| Camera/image resources | Planned frame byte/item bounds and current-frame ownership | B/C | Frame storage is reusable after consumer retirement; typed dimensions and one-frame payload size remain semantic resource bounds. | browser camera realization tests; #3551 owns continuous Vision expansion | classified / owner open |
| Patchbay browser presence/return workers | Fixed simultaneous worker, decision, and proof slots | B/D | Completed workers release execution slots; retained proof stays bounded and must disclose omission rather than grow. | #3619 / #3620 owner paths and existing capacity tests | classified / owner open |
| Front-door candidates, Forms, and topology | Fixed current inventory and visible topology slots | B/C | These bound one current reviewed workset/topology, not elapsed interactions. Removal/review creates a new current revision rather than silently overwriting it. | front-door capacity and revision tests | classified correct |

## Search record

The audit searched production Rust and browser modules for explicit exhaustion
messages, `next_*` and `*_count` comparisons, pressure/error variants, fixed
pending options, and code named ring, queue, slot, pool, window, transcript,
and buffer. The table records every confirmed small monotonic lifetime quota
from that pass and the representative legitimate-bound families needed to keep
the result reviewable. Broad numeric overflow checks and fixture-only counts
remain governed by classes E and F; they are not candidates merely because
they are finite.

The audit remains open while its owner issues are not admitted to `dev` and
the native WebRTC replacement-Plan path remains unresolved. Closing the owner
issue requires refreshing this ledger against the then-current tree and
rerunning its cited stresses; local commits or a larger constant are not
completion.
