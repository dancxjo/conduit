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
- exact aggregate value-slot, value-byte, evidence-item, and evidence-byte
  budgets; and
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

## Still open

The real std host still prepares and executes placements with the old
allocation-heavy `HostRuntime` operation pump. The next checkpoint must install
the lowered tables into a hosted kernel scheduler, adapt the signal operations
to the public kernel `Operation` protocol, issue and reverse-map active-play,
host-operation request, presentation, and evidence identities, and run
`flow/pulse -> presentation/show` through that path.

After that, the complete multi-value `tick -> tee -> filter/latest -> show`
form must run through the same std-host path before the old independent pump can
be removed.

## Proof commands

```bash
just check-kernel-takeover
just check
```

WASM, browser, socket, firmware, and physical tests do not prove this gate.
