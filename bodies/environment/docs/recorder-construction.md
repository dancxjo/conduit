# Recorder construction

This describes the prepared software contracts. The real browser recorder,
its durable catalog seed custody, Line and Body installation remain incomplete.
The public conformance entrance is `cargo xtask prove environment recording`.
The browser publication boundary is checked separately with
`cargo xtask prove environment recording-storage`.
The actual IndexedDB boundary has a separate focused entrance:
`cargo xtask prove environment recording-browser` (pinned Chromium required).
It produces artifacts under `target/environment-recording-browser` and retains
browser failure traces under `proof/browser/test-results/environment-recording`.

## Meaning and readiness

`install_recovery_kind` and `install_generation_source_kind` register meaning
with the ordinary Source checker and expansion catalogs. They initialize no
provider and create no Host, Boot, storage authority or generation.

`ReadyRecordingRecovery` owns an initialized recovery Back for one matching
gear in the checked workload. Root supplies the recorder's actual current
Host/Boot separately from the authenticated historical producer context.
`ReadyRecordingGenerations` owns a finite ordered list of already selected
portable references, with all encoded output reserved before Play. It creates
no reference and performs no storage lookup. An empty list closes normally.

Both use the same `ReadyRecordingSoftware` custody: advertise only while the
actual initialized Back is retained, check exact Source/Host/Boot/generation
and realization selection, then transfer that object once. A rejected selection
keeps the object. Root must replace historical advertisements after transfer;
an old descriptor is not continuing availability. These transfers do not start
Body/Play or authorize storage.

## Exact storage access

The authored `recovery.conduit` composes one generation Value, the existing
snapshot-read Value, an explicit `flow/singleton`, and the closing recovery
Flow. `RecordingRecoverySource` checks and expands this composition;
`PreparedRecordingRecovery` uses the ordinary planner and lowering, with
512-byte reference and 4 KiB content queues. The selected read must accept
the exact reference version as well as independent residence and authority.
`ReadyRecordingSingleton` retains the initialized temporal bridge before
advertising it. Its committed emission preserves the original value reference.
It owns no storage or recovery state.

The planning tests use explicitly identified CPU store and authority fixtures.
They check the actual retained software selections, absent authority/resource
refusals and a mismatched generation. The ordinary lowered plan also executes
through the production kernel with those transferred software objects: one
exact reference request, no recovery progress while the read is pending,
and committed recovery only after valid completion. Failed, cancelled and
invalid-content reads preserve empty history. Execution allocates no storage.
That earlier external read completion remains a CPU fixture. The additional
Linux file/kernel proof below replaces it with actual file effects.

The additional `cargo xtask prove environment recording-files` proof executes the
ordinary Source-checked, planned and lowered production recovery kernel against
actual local Linux snapshot files. The kernel's original request and input value
reach `PreparedFileSnapshotOperation`; validated output is stored in the kernel's
own value store and completes that same request. History remains empty while
I/O has completed but the kernel request has not, and until the ordinary sink
subsequently steps and commits. The successful chunk preserves first-class
quantity bytes and an explicit checksum failure. Missing files, invalid JSON in
a valid record envelope, and valid JSON with invalid recording content all leave
history empty. All four execution paths allocate no Rust heap during Play.

`PreparedFileRecovery` now retains that kernel and file executor together. Its
execution methods reject a foreign Play before stepping or dequeuing work.
The fifth file test obtains each current Play from a retained CPU Body binding:
the loan refuses before start and after Lull. Reserved-scope cancellation remains
available for cleanup. This tests lifecycle wiring, not live Host admission.

Linux and browser recovery use the same `PreparedRecordingKernelFace` projection
in the environment runtime; the browser name remains a public alias. Linux
projects its owned single-generation kernel, while the browser projects its
batch kernel. Both read the actual recovery Back's committed range. The Linux
file test keeps the Face empty after file completion until ordinary sink commit,
then checks an idempotent, allocation-free projection and its HTTP Mask output.
Neither projection owns a second history counter or acknowledges a new write.

Store offers/grants, historical producer context and the independent publisher
binding are explicit CPU fixtures. This proves production-kernel/file effects,
not live Host capacity admission, a real Body start, the MCU Line, durable
catalogue seed custody, Pi boot/display or physical overnight recording.

Browser snapshot publication validates bounded JSON text without constructing
the canonical JSON value tree. This admits a full 64-sample chunk's 2,688-byte
hex field within the existing 4 KiB envelope and eight-value nesting ceiling.
The same parser still enforces the tighter string, collection and numeric
bounds when constructing `JsonValue`; snapshot storage preserves the raw text.
The browser kernel regression reaches its ordinary storage request with a full
chunk-sized field. A separate test feeds the actual encoder's full 64-sample
chunk, retaining first-class temperature and humidity Forms, through the text
validator without allocations. This establishes the publication boundary.

The [overnight browser proof receipt](../evidence/2026-10-23-overnight-browser-storage/measurement.json)
records 15 full chunks plus their selected-generation catalog through the
production snapshot envelope and existing storage adapter into real IndexedDB.
After a page reload, Rust validates the catalog and uses its decoded exact
references to restore ordered history: all 960 samples over 16 logical hours,
including 904 exact canonical observations and 56 explicit sensor failures.
Each record rejects duplicate publication, stale effect scope, old authority
and corruption; an absent generation remains `ResourceMissing`. The maximum
16-entry catalog boundary also fits the ordinary 4 KiB snapshot contract.

Identities, authority, preselected browser read bindings and the catalog's
starting reference are fixtures. This proves durable batch and catalog bytes
across reopen. The subsequent kernel/IndexedDB proof below connects actual
reads to original kernel requests. Durable Root custody of that starting
reference, Body installation, a production Line and physical overnight operation
remain incomplete. The [earlier single-chunk receipt](../evidence/2026-10-23-browser-recording-storage/measurement.json)
retains its narrower historical scope.

`PreparedRecordingKernel` prepares this finite four-gear production kernel from
the verified fragment and identical ordinary lowering. It rejects unsupported
shape/storage and stale tables, preflights all retained software selections,
then transfers those exact initialized objects. An unavailable owner refuses
before any other owner is consumed. Its generic read driver only crosses the
Host Call boundary: Root must separately retain the selected storage executor,
admit the complete memory/work closure and confirm the real Body start.

The browser's `PreparedSnapshotOperation` is shared by the inline runner and
other kernel driver sets. It prepares one exact placement's record validation,
checks the lowered node/call and byte bounds, and retains one pending request.
Overlap, replay and mismatched completions refuse without replacing that
request. Storage bytes become an output only after validation; the caller still
completes the original kernel request and retires the effect separately.
The existing IndexedDB adapter remains the external storage executor. This
prepared operation neither executes IndexedDB nor admits a recorder Body.

The generation source emits `resource/ref` values, matching the existing
snapshot-read Fore. Its cursor advances only after the kernel commits an
emission. Output pressure, insufficient fuel or an uncommitted Step cannot
skip a generation; cancellation stops emission. Preparation rejects duplicate
identity/version pairs, incompatible profiles and an exceeded finite ceiling.

A portable reference is data. Snapshot reads still need independently admitted
current residence, exact resource bindings and authority. The existing browser
snapshot Back binds one exact generation; a list cannot silently broaden that
binding. A complete batch needs the corresponding admitted read routing.
Root must also obtain and retain its ordered reference catalog through admitted
state. This source does not discover a catalog, reserve manifest slots or prove
that referenced content has been published.

The Linux `PreparedFileBatchRecovery` now owns the ordinary batch kernel with
one prepared actual file executor per selected read. It rejects a foreign Play
before dequeue or Step and prepares all file bindings before transferring any
retained software. The focused `recording-files` proof publishes and reopens
1, 15 and 16 full chunks, then recovers through original kernel requests without
execution allocations. The 15-chunk case recovers 960 samples, including typed
quantities and explicit checksum failures. A missing middle file terminates
without fabricating later history. These are accelerated CPU executions using
actual disk effects and fixture offers/grants, not physical overnight evidence,
durable Root catalog-seed custody or live Host capacity admission.

## Ordered batch Source

`RecordingBatchRecoverySource` compiles the catalog-selected ordered references
into ordinary checked meaning. Each reference has its own snapshot-read Value
and explicit singleton Flow. The existing `flow/concat/finite` law drains each
left stream before forwarding the next, ending at one shared recovery sink.
The [checked Source receipt](../evidence/2026-10-23-batch-recovery-source/measurement.json)
records 60 gears and 59 cords for 15 generations; the declared 16-generation
boundary checks at 64 gears. Empty, duplicate, incompatible and oversized
reference sets refuse. Changing reference order changes the checked result.

This compiler creates no provider, residence, authority or start.
`prepare_generation_for` and `prepare_singleton_for` initialize separate retained
software for an exact checked gear. `ReadyRecordingConcat` retains the existing
ordered concatenation Back with the same checked terminal contracts. The legacy
unique-gear constructors still refuse an ambiguous repeated Kind.

`current_selection` exposes a retained owner's last advertised choice only while
its Back remains available. Root supplies those exact current per-gear choices,
actual storage residence and independent grants to
`PreparedRecordingRecovery::prepare_batch`, which uses the ordinary Planner.
Every read must match its configured generation; missing authority, swapped
generations and ambiguous Base/pool provenance refuse. Rejected choices consume
no software; successful transfer removes that owner's reported availability.
The [batch Plan receipt](../evidence/2026-10-23-batch-recovery-plan/measurement.json)
checks 45 actual retained software owners and 15 explicit storage fixtures.

`PreparedRecordingBatchKernel<N, C>` transfers those preflighted owners into
the same production fixed kernel, with two port ordinals, at most 64 gears and
63 cords. It prepares all canonical value slots and local Sign backing before
execution. Native Sign storage covers both the portable item and byte budgets.
The boxed preparation entrance avoids a large scheduler return value on the
caller's stack. Each read still crosses the generic admitted Host Call boundary;
this kernel owns no external storage executor.

The [batch kernel receipt](../evidence/2026-10-23-batch-recovery-kernel/measurement.json)
executes 1, 15 and 16 chunks. Reads remain pending without moving history, then
complete in reverse dispatch order while the planned concatenations preserve
Source order. Mixed observations and sensor failures recover through the whole
history with zero execution allocations or reallocations. Failed, cancelled and
invalid-content reads refuse without publishing history. Stale tables and an
unavailable owner refuse before other software transfers.

The [kernel/IndexedDB receipt](../evidence/2026-10-23-kernel-indexeddb/measurement.json)
then connects original pending kernel requests to actual Chromium storage reads
after page reload. Reverse completion recovers all 960 samples without execution
allocations; equal node-local sequences cannot interchange placement scope.
Files carry the proof transport between native kernel and browser. Root must
still admit the complete memory/work closure, retain providers through the real
Body start, and use production transport. The original single-generation kernel
retains its separate finite profile.

## Batches, recovery and Face

`PreparedRecordingBatch` initializes every encoder, publication Back and commit
observer for the complete finite workload. The default 960 samples occupy 15
full 64-sample chunks. Root supplies each immutable generation and its exact
content extent. Partial flushes require separate admission. The constructor
makes no store live and owns no implicit retry, next generation or eviction.

A `RecordingSnapshotBack` advances its observer only through an actual committed
Host Call completion. `RecordingRecoveryBack` similarly exposes history only
after committed consumption of valid content in the expected producer context.
Bad content, cancellation and capacity refusal preserve previous recovery.

`PreparedRecordingFace` projects committed recovery counts, sequence range,
known gaps and refusals into the ordinary Face grammar. The HTTP
`recording_response` Mask formats that sealed Face and retains its identity and
revision. It keeps no second recorder state. Recovered history is distinct from
a new durable write acknowledgement and from fresh sensor observation; typed
Temperature and humidity remain in the observation/history contracts.

CPU conformance covers Source/Plan selection of initialized software, exact
ownership transfer, transactional progress, recovery and Face publication.
It does not establish IndexedDB persistence, a live Line, host membership,
physical overnight recording or complete MCU memory admission.

The [32-bit resident-canonical-bytes receipt](../evidence/2026-10-23-resident-canonical-bytes/measurement.json)
records reduced appliance preparation storage with all seven finite overnight
scenarios preserved. Make emits static canonical bytes alongside independently
pinned computation text. Runtime checks their exact correspondence before any
initialization; altered or truncated bytes refuse without allocation. This
trades firmware storage for removal of temporary expression hex-decode buffers.
Retained software requests 61,926 bytes and preparation peaks at 62,285 bytes.
Software preparation alone now fits the current 65,536-byte Root arena.
Complete MCU admission must also cover allocator overhead, Root, native owners,
value/sign backing, kernel and stack. The physical firmware has not changed.

## Finite selected-generation catalog

`PreparedRecordingCatalog` retains up to 16 ordered, exact resource-ref v1
references in one ordinary JSON snapshot. The 15-chunk overnight workload needs
16 storage records including this catalog. Preparation checks both the complete
content-byte ceiling and that extra record; these checks are constraints, not
storage reservations. Root must select and publish the catalog's own generation
and retain its starting reference through admitted durable state separately.

`PreparedRecordingCatalogReader` validates the complete bounded representation
before replacing its previous selection. Reads allocate nothing; malformed,
duplicate or incompatible references preserve the prior catalog. It returns
canonical reference bytes, leaving owned decoding to subsequent preparation.
A selected reference does not acknowledge publication, prove residence or grant
a read. Missing listed chunks must still refuse through the actual snapshot
Host Call; each recovered chunk must satisfy its expected producer context.
This catalog has not yet been connected to the live recorder Body or Line.

## Production batch software preparation

`ReadyRecordingBatchSoftware::prepare` initializes and retains each exact
generation, singleton, finite concatenation and recovery Back before publishing
its offer to Root's Host advertisement. Root supplies only the existing storage
read choices; the constructor neither synthesizes snapshot providers nor issues
grants. The ordinary planner still checks resource provenance and independent
authority. Failure, including a late offer collision, leaves the supplied Host
advertisement unchanged.

The returned owners and their actual advertised choices pass directly to
`PreparedRecordingRecovery::prepare_batch` and then to the production batch
kernel. Ownership transfer removes their continuing availability. This is now
the same preparation path used by the 1-, 15- and 16-generation kernel proofs
and the actual IndexedDB/kernel proof. `cargo xtask prove environment recording`
checks missing, foreign and unoffered read choices plus late publication failure;
`cargo xtask prove environment recording-browser` checks the real storage I/O
connection after this production preparation. Actual browser Body admission,
production transport and durable catalog-seed custody still remain.

## Recorder lifecycle binding

`PreparedRecordingRecovery::propose_to_body` submits the exact recorder Plan and
explicit Mask topologies to the ordinary Body owner. Current membership and the
complete workset must match; a proposal retains Wake evidence without starting
Play. Failure preserves the owner's evidence and realization.

`NativeBodyBinding::prepare_recording` reuses the native appliance's existing
custody gate for the distinct Body and fragment Play identities. Root supplies
both prospective identities from its own issuers. Confirmation requires the
exact start retained by the current `BodyLifecycleSession`; proposals, foreign
Host/Boot/Plan/Play substitutions and a revoked binding refuse. Identity access
checks allocate nothing and stop succeeding after Lull. This is lifecycle
correlation, not resource admission, an execution claim or provider revocation.
Root must check its current owner before stepping or dispatching effects and
retire the actual providers and kernel when lifecycle ownership ends.

The public recording proof includes these refusal cases. The actual IndexedDB
proof now uses the same binding before dispatch and completion, then checks
Lull invalidation. Membership and attributable start are still explicitly CPU
fixtures; actual browser admission and production transport remain unfinished.

## Committed recovery Face in the browser realization

`PreparedRecordingBrowserFace` binds a fresh existing `PreparedRecordingFace`
to the exact selected recovery placement, ordinary lowering and fragment Play
provenance. It borrows the production recovery Back's committed range and feeds
that range into the existing Face projection. It stores no parallel progress
counter. Pending storage and validated but unconsumed completions leave the Face
unchanged; repeated projection of the same committed range is idempotent.
Root may publish an observed refusal through this same Face. No failure is
inferred merely because progress is absent.

The HTTP Mask consumes this exact Face. The actual IndexedDB proof checks an
empty Face while all reads remain pending, then one revision for the committed
960-sample recovery and allocation-free HTTP formatting. The focused storage
proof also checks foreign Face provenance, a non-recovery node and corrupt input.
Root still owns the exact kernel/Face pairing and current lifecycle checks; this
projection creates no Body admission, transport, authority or live web page.

## Native selector storage reduction

The capture selector now uses Core's existing bounded borrowed canonical
validator and requires the independently prepared exact Type prefix. It retains
no second owned input schema or validator prefix. Its output workspace follows
the complete checked Type bound within the original admitted byte ceiling.
Wrong Type identity, malformed primitive data, truncation, trailing bytes and
capacity refusal remain explicit, and refusal preserves subsequent valid use.

The [32-bit selector receipt](../evidence/2026-10-23-borrowed-selector-storage/measurement.json)
measures retained software falling from 61,926 to 53,396 bytes, with preparation
peak falling from 62,285 to 60,116 bytes. All seven finite overnight scenarios
and the selector refusal test allocate nothing during execution. The ESP32
native Root cross-build passed. These requested-byte measurements still exclude
the full allocator/Root/kernel/stack closure; no physical climate admission or
new firmware deployment is claimed by this optimization.

## Native monotonic wait boundary

The CPU climate proof's direct timer completions are distinct from native timer
ownership. `mechanisms/io/monotonic-wait` now supplies the selected native
Host Call boundary for the existing Tick/wait contract. It retains actual
provider possession, checks exact Root-pinned Tick configuration, duration and
request sequence, and revokes an unfinished capability when a pending future
is dropped. Waiting emits no value. Core now admits a zero-result capability
without permitting nonempty completion or removing positive work/lifecycle
bounds. Existing unconfigured operation binding remains unchanged.

`cargo xtask prove environment wait` checks those contracts with explicit CPU
issuance and timer fixtures. The [native wait receipt](../evidence/2026-10-23-native-wait/measurement.json)
records allocation-free invocation/cancellation, shared pulse regressions and
the RP2040 no-std cross-check. Native ready advertisement, the actual ESP32
provider, assigned installation and full climate admission remain to connect.
