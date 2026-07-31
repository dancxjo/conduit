# Bounded FlowPolicy algebra current form

Status: normative current contract

FlowPolicy algebra schema marker: `0`

## Purpose

Backpressure is a semantic contract. Every live cord resolves before execution
to exact finite item and byte capacity, one pressure policy, exact watermarks,
and any policy parameters or type-owned proofs it requires.

No queue implementation, scheduler, transport, or language runtime may add an
overflow slot, spill buffer, implicit sample interval, silent loss, or
queue-library policy.

## Exact capacity and accounting

`FlowCapacity` contains three positive, ordinary integer limits:

- `items`: maximum resident logical values;
- `max_value_bytes`: maximum accounted bytes for one value; and
- `max_queued_bytes`: maximum sum of accounted bytes for resident values.

`max_queued_bytes` must hold at least one `max_value_bytes` value. It may be
smaller than `items * max_value_bytes`, allowing a tighter aggregate budget.
Neither an integer maximum nor another sentinel means “unbounded.”

The implementation boundary that creates a value reports its exact accounted
size. The owning TypeContract defines what is included: payload, required
metadata, and any representation overhead whose memory is charged to the cord.
Variable-sized values always have the per-value bound above. An offer exceeding
it is rejected before acceptance and evidenced.

Occupancy is the count and byte sum of resident queue values only. A value
returned as blocked/pending or rejected remains owned by the caller and is not
hidden queue occupancy.

## Watermarks and fairness

`FlowWatermarks` satisfies:

```text
0 <= low_items < high_items <= capacity.items
```

Pressure enters when an accepted value reaches the high watermark or an offer
cannot fit the item/byte limits. It clears after dequeue when occupancy reaches
the low watermark. This hysteresis is exact plan data.

current form blocking fairness is FIFO. A blocked producer retains its offered
value. Removing capacity emits `producer-ready`; the scheduler resumes blocked
offers in arrival order. A cord has one producer endpoint and at most one
outstanding blocked offer from that endpoint; a scheduler does not issue the
next offer until the prior one is admitted or returned. This contract does not
authorize an auxiliary wait-value queue.

## Pressure policies

For capacity 2 containing `[a,b]`, arrival `c` has exactly these current form
results:

| Policy | Full-buffer result |
|---|---|
| `block` | caller retains pending `c`; queue remains `[a,b]` |
| `reject` | caller receives rejected `c`; queue remains `[a,b]` |
| `coalesce(relation)` | provider-selected named target is replaced; for target `b`, queue becomes `[a,c]` and replaced `b` is returned |
| `sample(every,offset)` | arrivals outside the exact sequence schedule are ignored; a selected arrival that is full is also ignored |
| `drop-disposable` | incoming `c` is dropped only after exact type proof |
| `disconnect` | `c` is not accepted and the cord becomes disconnected |
| `fail` | `c` is not accepted and the affected scope becomes failed |

There is no `drop-oldest` policy. Coalescing names a domain-owned relation and a
provider implementation selects one logical queue target. An absent or invalid
target rejects the arrival; it does not guess. The old value is returned to
the caller, so destruction is not hidden.

There is likewise no shape-only “latest value” shortcut. A latest-state type
uses an explicit replacement relation and the coalescing policy. “Drop newest”
is admitted only as `drop-disposable` after semantic proof, never as a generic
queue operation.

Sampling uses zero-based arrival sequence:

```text
selected(sequence) = sequence mod every == offset
```

`every` is positive and `offset < every`. Non-selected arrivals are evidenced
even when capacity is available. A schedule with no period is invalid rather
than acquiring an implementation default.

`drop-disposable` drops only the incoming value. It never removes an already
accepted value to make space.

## Type-owned prerequisites

`FlowTypeFacts` preserves three outcomes:

- disposability is proven, disproven, or indeterminate; and
- coalescing relations are an exact known set or unavailable.

`drop-disposable` is compatible only with proven disposability. A disproven
trait is incompatible; an unavailable provider is indeterminate.

`coalesce(relation)` is compatible only when the exact relation appears in the
provider-declared set. A known set without it is incompatible; unavailable
relations are indeterminate.

The hosted `TypeRegistry` exposes these facts from the same exact immutable
TypeContract description used for compatibility. The built-in text type
disproves disposability and declares no coalescer. Missing providers do not
silently approve lossy policies.

The `sample`, `coalesce`, and `drop-disposable` policies permit semantic loss or
replacement. Resolution also requires both endpoint PortContracts to accept
TypeContract-defined loss. Lossless ports reject them before execution.

## Evidence

Every queue event has a monotonic local sequence plus post-transition item and
byte occupancy. The reference state machine emits, as applicable:

- `pressure-entered` and `pressure-cleared`;
- `value-rejected`;
- `value-coalesced(target)`;
- `value-sampled-out`;
- `value-dropped-disposable`;
- `consumer-ready` and `producer-ready`;
- `disconnected` and `failed`; and
- `cancelled(wake_producer,wake_consumer)`.

Every loss or replacement decision emits its corresponding event in the same
atomic transition. Rejecting a never-accepted value is distinct from losing an
accepted value, but remains observable. Exact execution-event envelopes and
provenance are owned by #12; the kinds and ordering here are their flow payload
contract.

## Allocator-free reference queue

`BoundedFlowQueue` uses caller-provided `Option<(T, accounted_bytes)>` slots.
Construction rejects storage shorter than the exact item capacity or
non-empty storage. Construction also assesses the supplied exact type facts,
so a coalescing or disposable-loss queue cannot be created from incompatible
or indeterminate proof. The queue never allocates and never owns more values
than those slots.

Offers return ownership explicitly as enqueued, pending, rejected, coalesced
with the replaced value, dropped, disconnected, failed, or already terminal.
Dequeues preserve FIFO order except for the one explicitly named coalescing
target.

An empty pop records one waiting consumer. A full blocking offer records one
blocked producer while returning the actual value. Specification 008 freezes
natural completion and draining cancellation as value-preserving drains.
Aborting cancellation transfers every queued value to caller storage and emits
loss evidence before terminal cancellation. Every cancellation explicitly
wakes pressure waits.

## Source and plan resolution

The `.panel` seed grammar supports:

```text
cord source.value -> sink.value {
    capacity = 8
    max_value_bytes = 65536
    max_queued_bytes = 524288
    low_watermark = 4
    high_watermark = 8
    pressure = block
}
```

When omitted, the seed language has exact schema defaults: 8 items, 65,536
bytes per value, aggregate `items * max_value_bytes`, high watermark equal to
capacity, low watermark one below high, and FIFO block. Resolved plans always
carry the resulting exact values. Reference examples spell every bound.

Coalescing additionally requires `coalescer = namespace/relation`. Sampling
requires `sample_every` and optionally `sample_offset` (default 0). Missing
relation or sampling period is a source error.

`PlanCord.flow` records the exact `FlowCapacity`, `Pressure`, policy parameters,
and `FlowWatermarks`. Invalid capacity, invalid watermarks, unavailable traits,
and endpoint loss conflicts fail resolution before execution with stable
`CND-FLW-*` diagnostics.

## Fixtures and properties

`conformance/c2/flow-policy.tsv` freezes the full `[a,b] + c` transition for
every policy, exact evidence ordering, resulting queue/state, and compatible,
incompatible, or indeterminate type-fact outcomes.

The Rust reference also exhaustively checks all 8-operation offer/pop traces
for each policy over a fixed capacity. After every operation:

```text
occupancy_items <= capacity.items
occupancy_bytes <= capacity.max_queued_bytes
```

Additional properties assert FIFO order where promised, evidence for every
loss, exact sampling selection, invalid-bound rejection, and cancellation wake
behavior.

## Normative requirements

| ID | Obligation |
|---|---|
| FLW-001 | Resolve every live cord to positive finite item and byte bounds |
| FLW-002 | Never admit a value beyond item, per-value, or aggregate byte capacity |
| FLW-003 | Record one exact pressure policy and every required parameter |
| FLW-004 | Keep blocked values with callers and apply FIFO wake fairness |
| FLW-005 | Require exact domain proof for coalescing and disposable loss |
| FLW-006 | Preserve indeterminate when the required provider fact is unavailable |
| FLW-007 | Emit ordered pressure, loss, coalescing, terminal, and wake evidence |
| FLW-008 | Never infer a sample interval, coalescer, overflow slot, or spill buffer |
| FLW-009 | Reject lossy policies when either endpoint requires lossless flow |
| FLW-010 | Wake blocked producers and consumers on cancellation |
| FLW-011 | Preserve FIFO ordering except for an explicit coalescing target |
| FLW-012 | Keep the portable reference queue allocator-free |
