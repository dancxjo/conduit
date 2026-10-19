# Shared immutable Fore delivery

The ordinary owned-adapter Flow path previously copied a payload into the report
and cloned it again for the adapter. It also cloned stable port and Kind
identities per output. This change prepares one immutable identity per exact
lowered port and shares one immutable payload between report and adapter.
Ownership transfer still does not acknowledge an output.

The supported capture entrance is `cargo xtask prove runtime-performance` with
`--hosted-pipeline true --values 1000000 --repetitions 3 --decisions 1000
--samples 1000 --output PATH`. Each output directory must be new and outside
tracked source. Both public captures passed optimized conformance and retained
clean source before and after execution. The before source is
`99dfe6eceeb9f8e318e388a06beb98382746c647`; the measured after source is
`f2bb3b0b9640c8a9f2ccd2626af817538706c7e5`, retained on
`perf/shared-fore-measured-20261019`.

## Allocation and copying

All three independent process pairs preserve the exact Source digest, ordered
output digest, ordered numeric kernel events including their sequence and
request fields, scheduler work, and store capacities. Each process includes
three repetitions at one, eight and 32 Gears: 27 before/after comparisons.
Each trial admits 64 ordered outputs. The window below spans the 63 outputs
between the first and last real adapter acknowledgements.

| Owned adapter window | Before | Shared delivery |
|---|---:|---:|
| Allocation calls | 378 | 63 |
| Requested allocation bytes | 2,898 | 1,512 |
| Reallocation calls | 0 | 0 |
| Deallocation calls | 0 | 0 |
| Payload materialization, source accounting | 1,008 bytes | 504 bytes |

The allocation reduction is 83.3%; requested bytes decrease by 47.8%. The copy
row counts two versus one eight-byte payload materializations for each of the
63 outputs. Source inspection and actual report/adapter pointer-equality tests
establish that accounting. It is **not a hardware memory-traffic measurement**.
These figures describe the output window, not the whole Host call or all Play
allocations. Flow still allocates its one retained shared payload per output.

The earlier optional borrowing path from [PR #6019](https://github.com/dancxjo/conduit/pull/6019)
used 189 allocations and 1,449 requested bytes in the historical debug fixture.
The shared path uses fewer allocations, but its Arc header raises requested
bytes by 63 in that small-payload comparison. That reference is historical;
the release pairs above compare the current owned-adapter parent directly.

## Throughput observations

After the two supported captures, the exact release executables were preserved
with SHA-256 digests. Six fresh processes ran on CPU 4 in alternating order:
before/after, after/before, before/after. No local compilation overlapped these
six runs. Each process still executed the entire original worker matrix.
The host was shared; there are no hardware counter measurements or isolation
claims. The public captures did overlap compilation and are retained separately.

Each table cell lists the three process-pair medians of the three repetitions.
Ratios are shared time divided by reference time; smaller values mean less time.

| Gears | Whole Host call ratios | First-to-last acknowledgement window ratios |
|---|---|---|
| 1 | 0.857 / 0.227 / 1.016 | 0.831 / 0.231 / 1.017 |
| 8 | 1.021 / 0.345 / 1.060 | 1.004 / 0.372 / 0.988 |
| 32 | 0.953 / 1.005 / 0.988 | 0.947 / 0.986 / 0.986 |

The public comparison was 10–16% slower on several wall-time medians. The pinned
process pairs are mixed and show substantial dispersion. In particular, the
second pair's one/eight-Gear reference trials spent much more CPU and wall time;
the cause is not established. Earlier native arithmetic controls in that pair
had a median CPU ratio of 1.007, which does not explain the later hosted windows.
No sample was removed. This evidence supports the deterministic allocation and
copy reduction, not a universal throughput gain or a certified confidence interval.
Whole-call measurements include preparation, Play, translation and teardown;
the completion window is a different scope.

## Ownership and bounds

`ExternalForeDelivery::new` constructs an owned immutable delivery. Its
`ExternalForeBytes` provides read-only slice access and preserves byte-array
serialization. Delivery clones share both payload and stable metadata. Callers
that need to mutate bytes make an explicit owned copy. Struct-literal callers
migrate to the constructor; public identity field reads remain available through
the read-only identity view.

Value and Body outputs prepare their payload buffers before Play. Filling a
prepared payload requires unique custody and checks its admitted extent; an
escaped buffer refuses instead of using copy-on-write or growing storage.
The output pool is tied to the exact borrowed lowered-port occurrence. Flow
and Current retain independent payloads with the same prepared port identity.
Read-only collectors retain shared bytes directly, including the Body replay
and exact Quantity consumers, so migration does not add collection copies in Play.

## Validation and remaining scope

On the measured implementation, workspace Clippy with all targets and the
hosted-pipeline feature passed with warnings denied. Fifteen focused std Host
tests and the complete 26-test runtime-performance suite passed. Three separate
allocation-test processes also passed at all three Gear counts, including
actual shared payload/identity pointers, refusal and cancellation. Both public
release captures separately passed the 26 optimized conformance tests.

The implementation was replayed onto development source
`94f8f6c0365eac469746397233b98e5d2d749a81`. The source bridge verifies all 41
owned files are byte-identical at refreshed product commit
`b5904cabcc83805d04b86d345155b7247f85838c`. That bridge does not equate the full
trees or replace the refreshed candidate's lint, tests and integration gates.
Refreshed candidate validation and stable release acceptance remain pending.

The [evidence directory](evidence/shared-fore-delivery-20261019/README.md) retains the complete public and six independent worker
captures, original logs, source/Plan/Host artifacts, custody receipts, executable
digests, comparison summaries and a standalone verifier. Fresh output paths
intentionally mint different Boot, Plan, active-Play and observation identities.
The verifier checks their correlations within each capture; it does not claim
byte-identical Plans across different Boots. It compares the nine ordinary
owned-adapter cases per pair; the other retained matrix cases are not silently
promoted to an equivalence claim.

This work does not implement persistent prepared sessions, compact numerical
regions, cross-host codecs, or final stable acceptance. Those audit requirements
remain separate work.
