# Kernel takeover integration gate

Issue [#389](https://github.com/dancxjo/conduit/issues/389) blocks further
browser, transport, and firmware expansion until the production std path runs
exact plans through `conduit-kernel`.

## Accepted first checkpoint: exact lowering

`conduit_runtime::lowering::lower_plan_fragment` accepts one verified local
`PlanFragment` and, before Play start, derives:

- numeric node IDs and fixed-width input-cord tables;
- separate numeric input/output port ordinals with a directional reverse map;
- item/byte-bounded cord specifications and direct fan-out route ranges;
- per-node numeric host-operation admission bindings;
- per-node numeric resource references retaining their exact pool bindings;
- numeric mandatory-sign targets;
- exact aggregate cord queue-slot, cord byte, mandatory-sign-item, and
  mandatory-sign-byte budgets; and
- a reverse identity map for plan, fragment, placement, port, connection,
  host-operation contract, and resource binding identities.

Lowering allocates and performs string/map lookup only before Play start. Its
result contains the tables the kernel can install without graph scans,
base selection, or heap growth while stepping.

This checkpoint deliberately rejects:

- fragments whose sealed identity no longer verifies;
- remote connections;
- more than sixteen inputs or outputs per node;
- more than one cord targeting an input port;
- host-operation concurrency other than one;
- malformed endpoints, ports, resources, and Sign references; and
- numeric or aggregate capacity overflow.

Those are integration limits, not claims that the corresponding forms are
invalid in the general model.

## Accepted std takeover

The unchanged two-node `flow/pulse -> presentation/show` local plan and its
three-sink local fan-out now:

- installs the lowered node, cord, route, and host-operation rows into one
  fixed-capacity hosted kernel scheduler;
- preallocates every signal and timer value before Play start;
- drives the public kernel `Operation` protocol for both placements;
- correlates requests by `(node, request)` so independent operation counters do
  not collide or permit stale completion;
- completes waits through the std timer adapter and presentations through the
  stdout adapter;
- binds a distinct active play and exact presentation/sign identities; and
- proves with an allocator probe that successful sealed Play start performs
  zero heap allocations and cannot re-enter graph, kind, base, or registry
  lookup.

The complete typed `tick -> tee -> filter/latest -> show` conformance form uses
the same std/kernel boundary with exact pressure, closure, cancellation,
resource, Sign, and identity proofs. The ordinary `conduit` CLI uses these
installed profiles. Unsupported std forms fail closed; production `StdHost`
does not contain `HostRuntime`, expose its command surface, or fall back to its
operation/connection pump.

## Compatibility boundary

Cross-host browser/Pico simulations are not production std execution. They opt
into `LegacyStdFixtureHost` by name until their later host/link milestones
migrate. Composite conformance likewise remains later work. Neither path is
present in default production `StdHost`.

## Proof commands

```bash
cargo xtask check kernel-takeover
cargo xtask check workspace
```

WASM, browser, socket, firmware, and physical tests do not prove this gate.
