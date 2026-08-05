# Salvage S1 kernel

Issue #349 replaces the reboot runtime rather than extending its broadcast
operation API.

## First slice

`conduit-kernel` is `no_std` by default and defines one allocation-independent
state-machine contract:

```text
OperationInput
  Value { port, value }
  Closed { port }
  HostOperationCompleted { request, outcome }

OperationAction
  Await
  Emit { port, value }
  RequestHostOperation { request, operation, input }
  Complete
  Fail
```

Ports, nodes, cords, requests, and host operations are compact numeric
identities produced by lowering before activation. `FixedRoutes` and
`FixedHostOperationBindings` are sealed lookup tables: emitting on one output
cannot broadcast to another output, and an operation cannot invoke an
unplanned host boundary.

`ValueStorage` and `EvidenceSink` enforce independent item and byte budgets.
The fixed profile uses const-generic arrays. The hosted profile allocates every
slot and maximum value buffer during construction and does not grow them while
storing values. Both profiles run the same conformance vectors.

## Deterministic scheduler slice

`FixedScheduler` activates only from sealed numeric route and input-cord
tables. It uses fixed per-cord ring slots with independent item and byte caps,
then advances ready nodes in deterministic round-robin order. A step stages
input consumption and output sends before committing them together. Fanout
retains the exact number of queue references; emitting on one port cannot
broadcast through another port.

Producer completion closes each outbound cord. A sink observes its own input
port as closed only after that cord's queue drains. Cancellation calls each
unfinished driver, releases queued and driver-owned stored values, clears the
ready set, records bounded terminal evidence, and makes later steps report the
terminal cancelled state.

The required multi-value pressure vector now runs through
`source -> tee.left -> filter -> show-a` and
`tee.right -> latest -> show-b` with capacity-one queues and uneven sink
consumption. Here `latest` is only a pass-through test driver for kernel
plumbing; it is not the future `conduit.std` state/latest semantic operation.
The fixed and hosted storage/evidence profiles produce identical normalized
decisions, outputs, closure, and cancellation evidence.

## Deliberate archive reuse

The slice reuses the archived scheduler's staged-port/fixed-storage concepts,
not its implementation. The old broad plan, provider, policy, registry, and
catalog layers were not copied. The reboot runtime remains a prototype during
the transition and no semantic kind has been adapted to the new kernel yet.

## Current stop line

This is not completion of #349. Still required:

- correlated pending host-operation storage and late-completion rejection;
- no-growth activation proof for the complete hosted executor;
- the final semantic tee/filter/latest implementation rather than kernel test
  drivers.

## Checkpoint

```text
cargo test -p conduit-kernel --features alloc
cargo check -p conduit-kernel --target thumbv6m-none-eabi
just check-kernel-s1
```
