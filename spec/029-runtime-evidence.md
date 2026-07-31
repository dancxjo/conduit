# Bounded runtime evidence current form

Status: C4 normative portable policy and hosted reference implementation

Runtime-evidence policy schema marker: `0`

ExecutionEvent schema marker: `0`

ExecutionPlan schema marker: `0`

Depends on: specifications 007, 008, 011, 012, 017, 022, 023, 025, and 028

## Boundary

This specification projects executor-owned runtime observations into the
immutable `ExecutionEvent` envelope from specification 012. It does not create
a second evidence model, parse logs, infer values from channel writes, or turn
telemetry into a normative run outcome.

The following remain distinct:

- scheduler observations in fixed executor storage;
- immutable normative `ExecutionEvent` records;
- the Resonance retention/subscription envelope;
- domain and control events;
- compatibility stdout/stderr `channel_chunk` records;
- rebuildable projections and Patchbay presentation.

Specification 080 owns semantic value envelopes, domain/clock conversion, and
feedback state. current form therefore records an executor-local opaque handle
only as observation metadata; it is not a semantic value identity.
Specification 083 owns deadline and service guarantees. Local latency ticks
here are observations, not a real-time promise. Specification 082 owns exact
external effect commit and resource-lease semantics; this recorder preserves
their required event categories without inventing commit points.

## Exact plan policy

current plan schema adds at most one `RuntimeEvidencePolicy`. The policy is part of
exact plan identity and contains:

- explicit `disabled` or `record` mode;
- the exact normative-evidence Resonance stream;
- maximum event count and accounted serialized bytes;
- event/byte capacity reserved from optional telemetry for required evidence;
- a deterministic telemetry sampling period and offset; and
- the accounted size of an explicit sampling summary.

Every current plan carries the policy. It can explicitly disable
instrumentation with no stream and
all bounds zero. Recording requires a current plan `EventStreamContract` whose
class is `normative-evidence`, whose terminal record is required, whose
retention/provider capacities cover the recorder bounds, and whose subscriber
flow does not permit loss. This uses Resonance's existing provider,
retention, cursor, coupling, gap, redaction, and replay contract; no
recorder-only bus or subscriber protocol exists.

Changing mode, stream, capacity, reserve, sampling, or summary accounting
changes plan identity. Registry order, wall time, CLI subscribers, and
presentation do not.

## Executor observations

The deterministic scheduler extends its fixed `SchedulerEvent` observation
with:

- optional current and related opaque value handles;
- exact readiness-to-selection scheduling ticks; and
- exact deterministic step-processing ticks.

It records accepted and consumed values at committed queue transitions.
Coalesce/reject/sample/drop/disconnect/fail observations retain the attempted
handle where available. A committed output with committed inputs adds bounded
derivation observations. No event is emitted for a staged transaction that
rolls back.

This metadata increases the fixed scheduler event size and is included in the
existing preallocation calculation. Filling scheduler evidence remains
terminal `CND-SCH-010`.

## ExecutionEvent projection

The hosted reference recorder accepts only an exact plan, a typed
`RuntimeEvidenceContext`, and a bounded slice of `SchedulerEvent`
observations. It accepts no `Read`, `Write`, byte-channel, log, or prose input.

The context names run, recorder, observer, local monotonic basis, and immutable
correlation. Each recorded event has:

- a contiguous recorder and observer sequence;
- exact plan identity;
- a deterministic event ID;
- the expanded run/node/cord subject;
- the longest exact logical composite ancestor, when one exists;
- one stable kind and detail;
- local monotonic observed time;
- explicit causation and derivation event IDs;
- no wall or domain time inferred from scheduler ticks; and
- exactly one final terminal outcome.

Resource and authority bindings are required events projected when the
corresponding prepared node has exact plan bindings. A resource event carries
the public plan resource-binding ID. An authority event's stable detail suffix
is its ordinal in the exact plan authority collection, while its payload is
secret and redacted; effect and grant material are never copied into the event.
Checkpoint, faceplate
operation, replicated-instance, host-service, and plan-transition
instrumentation use the same required path when their owning runtime features
arrive. They must retain request, authorization, attempt, lease, checkpoint,
generation, or epoch identities from their owning contracts.

### Stable projection

| Scheduler observation | ExecutionEvent family | Sampling |
|---|---|---|
| allocation, prepare, start | lifecycle | forbidden |
| value accepted/rejected/dropped/coalesced | matching value family | forbidden |
| value consumed or committed input/output relation | derivation | forbidden |
| pressure enter/clear | pressure | forbidden |
| cord drain/completion/disconnect/fail | lifecycle | forbidden |
| cancellation request/cord cancellation | cancellation | forbidden |
| resource/authority/checkpoint binding or change | matching family | forbidden |
| decision, wake, node outcome | progress telemetry | policy |
| run terminal | terminal | forbidden and exactly once |

Every pressure/loss event carries the resulting occupancy and accounted bytes.
Every handle remains explicitly executor-local. Derivation relationships use
immutable event IDs, not handle equality as semantic identity.

## Fixed observation payload

Scheduler-observation events carry a public 52-byte
`conduit/runtime-observation` current-form payload:

| Offset | Bytes | Meaning |
|---:|---:|---|
| 0 | 1 | payload version, `1` |
| 1 | 1 | presence flags for current/related handles |
| 2 | 2 | occupancy items, big endian |
| 4 | 8 | occupancy bytes, big endian |
| 12 | 8 | opaque current handle or zero when absent |
| 20 | 8 | opaque related handle or zero when absent |
| 28 | 8 | local scheduling latency ticks |
| 36 | 8 | local processing latency ticks |
| 44 | 8 | original scheduler-observation sequence |

This payload contains no represented value bytes, secret, resource credential,
authority material, domain measurement, or wall timestamp. Protected domain
payloads remain reference/redacted under specification 012 and Resonance;
subscriber presentation never mutates the immutable source event.

## Bounded admission and sampling

`RuntimeEvidenceBudget` performs allocation-free checked event/byte
accounting. Required nonterminal events cannot consume the terminal reserve.
If a required event cannot fit, recording fails closed with `CND-RTE-006`.
Required terminal, loss, pressure, authority, resource, checkpoint, transition,
and cancellation facts are never sampled.

Optional telemetry ordinal `n` is selected exactly when:

```text
n % telemetry_period == telemetry_offset
```

Skipped telemetry increments a checked counter. Before the next selected
telemetry record, or before the terminal record, the recorder emits one
`runtime/telemetry-summary` event naming the skipped count. If that summary
cannot fit, recording fails closed with `CND-RTE-007`; there is no silent
overflow. Subscriber gaps remain the separate explicit Resonance gap
mechanism.

Coupled subscribers can block execution only through the exact lossless
subscriber `FlowPolicy` in the plan. Isolated subscribers cannot block the
publisher and receive explicit retention/subscriber gaps. Neither subscriber
changes, deletes, or redacts the immutable source record in place.

## Time, correlation, and paths

Scheduling latency is measured from the exact local ready tick to selection.
The deterministic reference executor charges one local processing tick to a
selected step. Hosted executors may use another plan-authorized monotonic
basis, but must name it and must not substitute wall time.

Wall time is optional presentation/correlation metadata only. Domain/event
time comes from an authorized value envelope under issue #80. Timestamps from
different bases are not compared without a declared conversion. Distributed
order is carried by request/session/work/attempt/correlation/transport IDs and
causal event references, never inferred from timestamps.

Expanded subjects identify the exact primitive, cord, attempt, or run.
`logical_template` is the longest plan composite path that boundary-safely
contains the expanded subject. Both are immutable evidence facts.

## Terminal and explanation invariants

Every recorded run contains exactly one final terminal event. No event follows
it. A missing or duplicate terminal result is `CND-RTE-008`. Required loss,
pressure, cancellation, resource, authority, checkpoint, and transition facts
must precede it or recording fails rather than fabricating completion.

An operator can follow causal and derivation references from the terminal,
loss, or latency observation to exact node/cord paths and plan identity.
Mutable “current state,” UI labels, and human explanations remain projections.

## Machine output

Specification 028 provides the direct bounded `conduit.run`
`execution_event` record path. A caller serializes these owned immutable events
through that method. It does not route them through implementation
`Write::write`, parse `channel_chunk`, or wrap JSON text as a compatibility
channel value. The 65,536-byte outer record limit remains independently
enforced.

## Diagnostics

| Code | Meaning |
|---|---|
| `CND-RTE-001` | unsupported runtime-evidence policy version |
| `CND-RTE-002` | malformed or version-incompatible policy |
| `CND-RTE-003` | stream is missing for record mode or present for disabled mode |
| `CND-RTE-004` | stream class, loss, retention, provider, or terminal capability is insufficient |
| `CND-RTE-005` | event, byte, ordinal, or sampling arithmetic overflow |
| `CND-RTE-006` | required evidence exhausted its plan-visible capacity |
| `CND-RTE-007` | skipped telemetry cannot be summarized within capacity |
| `CND-RTE-008` | terminal evidence is missing or duplicated |
| `CND-RTE-009` | executor observation time or shape is invalid |
| `CND-RTE-010` | scheduler subject does not exist in the exact plan |
| `CND-RTE-011` | projected event fails the current ExecutionEvent contract |
| `CND-RTE-012` | bounded hosted allocation, handle index, or record-size accounting failed |

## Conformance

`conformance/c4/runtime-evidence.json` covers explicit disablement,
current plan identity, shared Resonance publication, lifecycle, pressure/loss,
occupancy, latency, derivation, logical/expanded paths, redaction, sampling
summaries, terminal persistence, and future ownership boundaries.

## Normative requirements

| ID | Obligation |
|---|---|
| RTE-001 | Project only executor-owned observations into current ExecutionEvent current |
| RTE-002 | Make recording mode, stream, capacity, reserve, and sampling exact current plan identity |
| RTE-003 | Reuse the normative-evidence Resonance profile without a recorder-only bus |
| RTE-004 | Keep scheduler, immutable evidence, domain/control events, channel chunks, and projections distinct |
| RTE-005 | Record lifecycle, occupancy, pressure, admission, loss, coalescing, cancellation, and terminal facts |
| RTE-006 | Record local monotonic scheduling/processing latency without cross-clock or deadline claims |
| RTE-007 | Preserve complete correlation, causation, derivation, logical, and expanded identities |
| RTE-008 | Never sample required terminal, loss, pressure, authority, resource, checkpoint, or transition evidence |
| RTE-009 | Emit an explicit bounded summary for every optional telemetry sampling gap |
| RTE-010 | Fail closed on required, summary, arithmetic, or fixed-storage exhaustion |
| RTE-011 | Retain exactly one final terminal event per recorded run |
| RTE-012 | Keep value bytes, credentials, and protected material out of the public runtime-observation payload |
| RTE-013 | Require the current runtime-evidence policy in every current plan |
| RTE-014 | Serialize runtime evidence through the direct bounded current run structured path |
| RTE-015 | Reserve semantic value/clock conversion for #80 and deadline guarantees for #83 |
| RTE-016 | Extend future pools, transitions, controls, jobs, and host services through the same required evidence path |
