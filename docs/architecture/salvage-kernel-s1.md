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

## Host-operation scheduler slice

A host-enabled scheduler is constructed with a sealed
`FixedHostOperationBindings` table and a const-generic pending-request array.
An operation can atomically consume inputs and stage one bounded host request.
Admission happens before queue/reference mutation; an absent binding, duplicate
or retired request identity, full pending table, or oversized input rejects the
step without consuming its inputs.

The host pulls a numeric `HostOperationRequest`, reads only its bounded stored
input, stores a budgeted outcome value, and completes the exact request. The
scheduler validates the output byte bound, keeps the waiting node asleep until
completion, then wakes it with the correlated outcome. Completion storage owns
the output reference until the operation consumes it. Completion before
dispatch, a wrong or repeated identity, and completion after cancellation are
rejected. Request identities increase for the lifetime of a run, preventing a
retired identity from being rebound to a later request.

Cancellation clears the fixed pending table and all run-owned values before
recording the terminal cancellation event. The fixed and hosted profiles match
for request, completion, decision, output, evidence, and terminal vectors.

## Deliberate archive reuse

The slice reuses the archived scheduler's staged-port/fixed-storage concepts,
not its implementation. The old broad plan, provider, policy, registry, and
catalog layers were not copied. The reboot runtime remains a prototype during
the transition and no semantic kind has been adapted to the new kernel yet.

## Current stop line

This is not completion of #349. Still required:

- no-growth activation proof for the complete hosted executor;
- the final semantic tee/filter/latest implementation rather than kernel test
  drivers.

## Checkpoint

```text
cargo test -p conduit-kernel --features alloc
cargo check -p conduit-kernel --target thumbv6m-none-eabi
just check-kernel-s1
```
