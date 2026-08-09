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
identities produced by lowering before Play start. `FixedRoutes` and
`FixedHostOperationBindings` are sealed lookup tables: emitting on one output
cannot broadcast to another output, and an operation cannot invoke an
unplanned host boundary.

`ValueStorage` and `SignSink` enforce independent item and byte budgets.
The fixed profile uses const-generic arrays. The hosted profile allocates every
slot and maximum value buffer during construction and does not grow them while
storing values. Both profiles run the same conformance vectors.

## Deterministic scheduler slice

`FixedScheduler` starts only from sealed numeric route and input-cord
tables. It uses fixed per-cord ring slots with independent item and byte caps,
then advances ready nodes in deterministic round-robin order. A step stages
input consumption and output sends before committing them together. Fanout
retains the exact number of queue references; emitting on one port cannot
broadcast through another port.

Producer completion closes each outbound cord. A sink observes its own input
port as closed only after that cord's queue drains. Cancellation calls each
unfinished driver, releases queued and driver-owned stored values, clears the
ready set, records bounded terminal Sign, and makes later steps report the
terminal cancelled state.

The required multi-value pressure vector now runs through
`source -> tee.left -> filter -> show-a` and
`tee.right -> latest -> show-b` with capacity-one queues and uneven sink
consumption. `latest` takes ownership of each input, releases the superseded
value, and emits only the final retained value after its input closes. The
kernel conformance driver proves the state/latest behavior but is not yet a
catalog-installed `conduit.std` operation; catalog installation belongs to S5.

`StepIo::take_input` transfers a consumed queue reference into bounded operation
state, while `discard` releases a previously retained reference. Those actions
participate in the same preflight/commit transaction as named outputs. A
separate two-input join vector proves that seeing only one side stages nothing
and leaves the first cord untouched until both sides can commit. The fixed and
hosted storage/sign profiles produce identical normalized decisions,
outputs, closure, join rollback, and cancellation Sign.

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
for request, completion, decision, output, Sign, and terminal vectors.
The hosted profile also records its value-slot, per-slot byte-buffer, and
sign-vector capacities at Play start and proves those capacities are
unchanged after a complete host-enabled run.

## Public operation adapter

`OperationDriver` adapts the published `OperationInput`/`OperationAction`
state machine into `FixedScheduler`. `Operation::advance` lets an operation
produce more than one named output for one input; the adapter collects those
actions in fixed arrays and publishes them as one scheduler transaction only
when every target is ready. Defaulted ownership hooks preserve the required
action vocabulary while allowing bounded state operations to retain a resumed
value and release one superseded value.

The fixed and hosted profiles match for a public-operation source/tee/two-sink
vector, including two tee emits committed from one input. A separate
host-enabled adapter vector proves that a public `RequestHostOperation` action
waits for and resumes from the exact correlated completion.

The final conformance vector drives four bounded host-generated tick values
entirely through `OperationDriver`:

```text
tick -> tee.left  -> filter -> show-a
        tee.right -> latest -> show-b
```

The tee publishes both named outputs atomically, filter admits two values,
latest retains and supersedes until closure, and both shows reach terminal
closure with no stored values or pending requests left. Fixed and hosted
profiles match outputs, decisions, Sign counts and bytes, closure, and
terminal state.

## Deliberate archive reuse

The slice reuses the archived scheduler's staged-port/fixed-storage concepts,
not its implementation. The old broad plan, base, policy, registry, and
catalog layers were not copied. The reboot runtime remains a prototype during
the transition and no semantic kind has been adapted to the new kernel yet.

## Acceptance boundary

This completes the S1 execution-kernel contract in #349. It does not promote
the reboot runtime or prototype catalog: no semantic kind is installed on the
new kernel yet. S2 must next bind exact semantic, implementation, resource,
authority, and observed-link facts before host or catalog expansion.

## Checkpoint

```text
cargo test -p conduit-kernel --features alloc
cargo check -p conduit-kernel --target thumbv6m-none-eabi
just check-kernel-s1
```
