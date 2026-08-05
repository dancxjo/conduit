# Kernel takeover integration gate

Issue [#389](https://github.com/dancxjo/conduit/issues/389) blocks further
browser, transport, and firmware expansion until the production std path runs
exact plans through `conduit-kernel`.

## Accepted first checkpoint: exact lowering

`conduit_runtime::lowering::lower_plan_fragment` accepts one verified local
`PlanFragment` and, before activation, derives:

- numeric node IDs and fixed-width input-cord tables;
- separate numeric input/output port ordinals with a directional reverse map;
- item/byte-bounded cord specifications and direct fan-out route ranges;
- per-node numeric host-operation admission bindings;
- per-node numeric resource references retaining their exact pool bindings;
- numeric mandatory-evidence targets;
- exact aggregate cord queue-slot, cord byte, mandatory-evidence-item, and
  mandatory-evidence-byte budgets; and
- a reverse identity map for plan, fragment, placement, port, connection,
  host-operation contract, and resource binding identities.

Lowering allocates and performs string/map lookup only before activation. Its
result contains the tables the kernel can install without graph scans,
provider selection, or heap growth while stepping.

This checkpoint deliberately rejects:

- fragments whose sealed identity no longer verifies;
- remote connections;
- more than sixteen inputs or outputs per node;
- more than one cord targeting an input port;
- host-operation concurrency other than one;
- malformed endpoints, ports, resources, and evidence references; and
- numeric or aggregate capacity overflow.

Those are integration limits, not claims that the corresponding forms are
invalid in the general model.

## Accepted second checkpoint: real std Signal execution

The unchanged two-node `flow/pulse -> presentation/show` local plan now:

- passes the old runtime only through an effect-free, temporary preparation
  validator;
- installs the lowered node, cord, route, and host-operation rows into one
  fixed-capacity hosted kernel scheduler;
- preallocates every signal and timer value before activation;
- drives the public kernel `Operation` protocol for both placements;
- correlates requests by `(node, request)` so independent operation counters do
  not collide or permit stale completion;
- completes waits through the std timer adapter and presentations through the
  stdout adapter;
- binds a distinct active play and exact presentation/evidence identities; and
- proves the hosted value and operation allocation capacities do not grow
  after activation.

The ordinary `conduit` CLI uses this path for the exact pair. Unmigrated wider
std forms explicitly retain the old runtime path rather than being silently
claimed as kernel execution.

## Still open

The complete multi-value `tick -> tee -> filter/latest -> show` form must run
through the same actual std-host path. Mandatory plan evidence, resource
reservation, pressure/cancellation/terminal projection, and full reverse
identity mapping must then be unified before the old independent pump can be
removed.

## Proof commands

```bash
just check-kernel-takeover
just check
```

WASM, browser, socket, firmware, and physical tests do not prove this gate.
