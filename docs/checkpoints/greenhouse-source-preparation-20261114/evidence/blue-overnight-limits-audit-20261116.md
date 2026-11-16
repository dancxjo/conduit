# Blue overnight acceptance limits — read-only audit, 2026-11-16

**The reviewed Source already describes a nominal16-hour sampling Play, but the live60-second proof does not establish an overnight run or available history capacity.** Its current append Capacity outcomes are an immediate retention blocker, separate from sampling/display continuity.

No builds, Source/config/grant changes, device operations or private-key reads. Inspected sealed `work/source-member-control-make-20261111/blue-record-service-linked-codecs-qualified` and `work/record-service-source-candidate-20261111/{yellow-record-service.conduit,blue-record-service.host.conduit}`, then the existing shared cadence/recording/Mask/continuation code.

## Exact artifact scope

Sealed build configuration is `sha256:97fc8423fb98a953d2882d35d8ee5802386ca73c354aa7a4927e39867f702976`; Make source identity `sha256:e4ecfe8efa79349fb9c2c8ea8a7a6edccf7e5b6c67ffd78786417b61e4b01e5d`; IMAGE `image:sha256:f35757b90995b19ab40a9fe2d1420231262ef2001a09cef11c469af7a125fb4a`. Actual `workset-source.conduit` SHA256 is `ec1b0410a8ca235e0b394625eede70c0d467cb1933c023c4f811b96db9e5f6c8`, independently matched to the sealed continuation recipe's source digest. These are different identity domains.

Selected entries are **yellow-sampling, yellow-mask, yellow-output**, despite the device being Blue. Those wrappers delegate to the ordinary recorded appliance/Mask/output; the names do not change pins, equipment identity or sampling semantics. The sealed build manifest itself says physical_deployment_performed=false; live Root evidence is separate.

## Finite clocks and operations

| Boundary | Configured limit / meaning |
| --- | --- |
| Sampling | `recorded-environmental-appliance` defaults count960, period-ms60000; wrapper supplies no override. |
| Cadence | `environment-sampling` selects `time/tick(count,period-ms)`. TickBack requests/completes a wait **before** producing each tick, then requests the next finite wait. Nominal last tick is16hours after start; scheduling/backpressure can delay it. This is not a guarantee of960 valid sensor readings. |
| Timer Base | maximum_duration_ms60000, slots4. This is a **per-wait duration/concurrent-owner bound**, not a60-second whole-Play lifetime or four-tick allowance. Existing equipment explicitly checks every selected Source period against it. |
| Pulse capture | Native policy derives its operation count from the total exact Source clock counts:960 captures for this single sampling clock. Electrical timing bounds remain per transaction. |
| Append operations | Source RecordingLog maximum_samples follows the finite checked clock events. `source_composition/play_recording.rs` adds the maximum context-fragment allowance:960+ceil(1536/160)=**970** admitted append calls. This is distinct from1264 physical media slots. |
| Read operations | Explicit checked record-service policy8192 read calls. Retries/refusals may consume operation/work budgets; this is not an unlimited polling grant. |
| Renderer / physical writes |65535 operations/frames, separately derived into original native operation policy. The page period is2000ms.16operation_slots in Host limits is concurrency/storage bookkeeping, not16 total calls. |

The renderer also has **show_lifetime_ms120000**. `plots/seven-segment-mask/src/format.rs::Alternation::select` rejects elapsed time at/after that lifetime; `prepared_show.rs` renders the explicit unavailable appearance outside it. It does not silently renew old readings. For overnight useful display, the local admitted pipeline must install fresh Shows as the Source publishes conditions, including appropriate unavailable/stale truth. One initial Show does not remain a current reading for12hours. This audit did not establish every actual fresh-Show renewal on live Blue.

At one page transition per2seconds,12hours requires21600 transitions; allowing one additional new-Show frame per minute gives roughly22320 frames.16hours similarly gives29760. Both are below65535, so the **declared frame count alone** is not the obvious overnight limiter. These are planning estimates, not worst-case execution bounds: extra Face updates, invalidations, retries and physical fault handling must be checked against the actual admitted pipeline. Do not enlarge the limit to hide unexpected frame churn.

## History duration and occupancy

The checked residence is8192 prefix bytes plus1264*512-byte slots =655360 bytes total. Each committed sample uses one immutable record slot; context fragments precede samples, at most10 under the1536/160 bounds. Failures/torn writes and previous Plays/Boots can consume slots too. Root recovery preserves occupied or damaged slots rather than overwriting them.

For a **fresh, fully available** residence:

* The existing960-sample Play needs at most970 new slots, leaving at least294 slots if every planned sample succeeds and there are no damaged/previous slots.
* A12-hour720-sample span requires at most730 slots under the same assumptions.
* Physical space alone conservatively permits1254 sample records after10 context slots:20h54m at one per minute. The present Play still ends at960 ticks/nominal16hours; journal capacity does not extend its execution count.
* Two complete12-hour runs cannot both fit without retirement:2*(720+10)=1460 slots. Every new stream/Boot consumes additional context and may leave damaged occupied slots after interruption.

These are append **capacity** calculations, not data-retention expiry: already committed records do not expire when the Play ends. They persist until an independently lawful storage lifecycle disposes of them.

Live `blue-root-continuity-20261115.json` reports recorded0/capacity4. As documented in the history audit, those are counters of four Capacity outcomes, not a four-slot device. They establish that current attempts were refused, not how many committed/uncertain/vacant slots remain. A recovered read scan and residence summary are needed to know current available slots. An overnight claim must start with enough actual remaining slots; checking the configured1264 is insufficient.

Fresh physical reconciliation: `blue-readonly-backup-20261115.json` and `blue-current-content-recovery-20261115.json` now establish the reviewed application match and **all1264 slots committed,1224 observations plus40 context fragments, zero gaps/corruption, zero remaining slots**. Those were recovered with the original production parser from the MAC-bound full-flash backup. This supersedes any uncertainty about occupancy from the capacity counters alone. No record was acknowledged, erased or reclaimed. The audit itself performed no new device I/O.

## Continuation is not an endless Play

The sealed recipe pins the exact three entries, Source/workset/IMAGE/configuration and placements. `ContinuationIntent` has local membership/intent generation checks and explicit finite assignment/runtime/durable/operation/active-workset ceilings. It carries **no blanket wall-clock TTL granting endless activity**, and no persisted Started token. Root must reconstruct and freshly admit the complete allowed workset against actual current owners on a new Boot. A continuation cannot enlarge960 ticks, renew spent capabilities in the same Play, reset media occupancy or erase history.

The public recipe/build does not alone prove that Blue's currently installed private continuation acceptance is valid and durable. That requires the existing actual acceptance/power-cycle evidence; no private Bootstrap was read in this audit. New-Boot local continuation also cannot infer remote revocation it has not received.

## Recommended >=12-hour gate without physical-resource growth

**Keep the reviewed960 ticks at60000ms.** It already provides nominal16hours, giving scheduling margin beyond12hours, within the existing one-minute wait ceiling and physical record count on an empty residence. No cadence change or new hardware grant is needed merely to express that duration.

If a deliberately shorter finite release is wanted, an authored count900 at60000ms gives nominal15hours and requires at most910 slots. Count720 gives exactly nominal12hours with less margin. Such changes are **not already configured**: the Source count is compiled into selected software definitions, clock operation ceilings, recipe/IMAGE identity and admission correspondence. They require ordinary fresh Make, reviewed Seed/recipe/continuation authorization, exact Owner replan/admission and deployment; they are not edits to a live assignment or counters. They can retain the current physical heap, timers, slots and frame ceilings, subject to complete preparation/resource requalification.

Do not set period above60000ms under the present timer construction. Do not increase count or frame/call allowances behind existing grants. No Source change can recover already occupied NOR capacity.

Before a truthful independent overnight release:

1. Restore lawful Root/owner continuity and retain the actual Boot/Source/Plan/Play receipt for all local fragments.
2. Read/reconcile existing history through the official typed service. Establish at least the required actual free slots; preserve context, sequence/time provenance and gaps. Current full-media Capacity is not solved by nominal960/1264 arithmetic.
3. Resolve media capacity only through independently owned lawful generation/block custody or a separately reviewed authorized reinstallation that preserves the required evidence. The existing read path and selected-recorder commitment do not themselves confer erase permission.
4. Run a bounded owner-disconnect observation covering multiple new Shows and >=one120-second Show-lifetime boundary; verify local fresh readings/explicit unavailable behavior, no electrical errors or excessive frame churn, and real successful media appends.
5. Then retain a>=12-hour independently powered run and a physical power-cycle/recovery proof. Short kernel progress, build completion and continuation recipe presence are not substitutes.
