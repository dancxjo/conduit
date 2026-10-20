# One reusable child for ordinary Thermostat Body Values

The ordinary authored Thermostat scan previously prepared 256 complete child
kernels before Play, one for each admitted command. Its outer Body kernel was
already long-lived. This change reuses one pure child kernel through the existing
bounded scan controller; the outer kernel is not optimization credit.

The supported path remains `Execution::new` / `execute` / `finish` in
`plots/thermostat/runtime`, using `main.conduit`, checking/expansion, the installed
Std Host, an exact Plan, sealed BodyPlan and ordinary live-Fore Body execution.
Todo and Parsec retain their existing child-pool policies. An unsupported reusable
factory refuses preparation; it never silently constructs a replacement kernel.

## Ownership and acceptance

Only a verified, single-fragment, one-operation, effect-free Value child is
eligible. Exact Source/Plan/Host/Boot/offer identity and the subordinate prepared
receipt still gate installation. Reuse is a separate factory opt-in. Process-local
Value boundaries must be admitted explicitly; the default prepared-session
constructor continues to reject remote boundaries. This does not reset network
sessions, epochs, transport ACKs or arbitrary Line endpoints.

One child kernel owns its Back, admitted storage and bindings. Before Play the
scan reserves all finite per-invocation Play identities and fresh Sign logs.
Completed logs, including remote lifecycle identities, move intact into retained
per-Value evidence. No mutable Back or previous evidence becomes a new invocation.
The prepared owner itself supplies the identity for rearm; an equal-looking Plan
in a different owner cannot substitute for it. Remote payload sequences continue
monotonically, while close Signs name the next sequence.

The scan exposes a candidate state under output pressure. It commits recurrence
and rearms only when the parent Back's output Step is committed. Rejected output
copies, repeated Steps, cancellation, stale identity, malformed input and exhausted
reservations cannot implicitly advance or replay a Value. Evidence for an unused
future invocation is not reported as an executed child. All 256 admitted Values
can complete without a 257th rearm.

## Allocation observation

The matched reference changes only the Thermostat installer back to the previous
finite child pool, with the same generic controller, source and observation code.
The global allocator observes both the caller and Body worker thread. A completed
primer settles process/thread initialization before each measured complete owner;
this is warm-process owner preparation, not first-process startup. The owner is
then constructed, executes 32 commands, finishes and materializes its report, is
inspected, and the report is dropped. Assertions check each exact Thermostat
state, normal completion, 32 child receipts, and retained event/sidecar counts.

Three alternating settled pairs produced identical phase counts (allocation
observations, not a timing benchmark):

| Boundary | Previous child pool | Reusable child |
| --- | ---: | ---: |
| Cold owner allocation requests | 508,231 | 344,783 |
| 32 warm Values allocation requests | 129 | 129 |
| Finish/report materialization allocation requests | 493 | 333 |
| Complete owner allocation requests | 508,853 | 345,245 |
| Complete owner requested bytes | 113,400,450 | 49,400,462 |
| Peak tracked live heap over baseline | 64,781,975 | 14,814,541 |
| Final tracked live heap delta | 0 | 0 |
| Retained child events / remote sidecars | 512 / 256 | 512 / 256 |

The saving is primarily avoiding unused complete child kernels at preparation.
Warm full-Body allocation requests are unchanged; they include channel/output and
product API work. The separate kernel/composite owner controls verify allocation-
free rearm and evidence transfer. All finite future evidence remains reserved:
this is not a claim that mandatory Signs became free or that memory is independent
of the admitted item bound. Heap accounting excludes thread stacks and allocator/
OS internals; no RSS, calibrated latency or broad product speedup is claimed.
The original unprimed observations retained a common 138-byte process allocation;
they are recorded separately from the settled complete-owner comparison. An early
probe used invalid non-five-unit target increments and refused before emitting a
cost record; the accepted probe uses canonical supported commands.

## Reproduce

Use repository Rust 1.98.1 on the pinned source in the observation record:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test -p conduit-thermostat-runtime --test owner_cost -- \
  --ignored --nocapture --test-threads=1
cargo test -p conduit-thermostat-runtime --test authored_kernel
cargo test -p conduit-composite --features fixture-registry-preparation
cargo test -p conduit-std-host --lib reusable_ -- --test-threads=1
```

`proof/performance/body/value-owner-20261020.json` pins sources, toolchain,
Host machine, phase observations and local worker custody. No private fixture or
hardware offer is needed. This slice does not claim shared main/release publication
or the full cross-workload baseline tracked separately.
