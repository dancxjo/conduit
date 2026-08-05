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

## Deliberate archive reuse

The slice reuses the archived scheduler's staged-port/fixed-storage concepts,
not its implementation. The old broad plan, provider, policy, registry, and
catalog layers were not copied. The reboot runtime remains a prototype during
the transition and no semantic kind has been adapted to the new kernel yet.

## Current stop line

This is not completion of #349. Still required:

- deterministic multi-node scheduling and per-port closure;
- bounded cord queues and transactional multi-input/multi-output steps;
- correlated pending host-operation storage and late-completion rejection;
- cancellation and terminal propagation;
- no-growth activation proof for the complete hosted executor;
- matching hosted/fixed vectors for the multi-value tee/filter/latest form.

## Checkpoint

```text
cargo test -p conduit-kernel --features alloc
cargo check -p conduit-kernel --target thumbv6m-none-eabi
just check-kernel-s1
```
