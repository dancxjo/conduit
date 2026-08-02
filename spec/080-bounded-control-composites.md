# Bounded request/reply and cancellable action composites

Status: normative current contract

Bounded-control catalog schema marker: `0`

## Scope and identity

`conduit.std/control/request-reply` and `conduit.std/control/cancellable-action` are reusable semantic
composites above the allocator-free core. They are made from ordinary typed
ports, finite correlation state, explicit clocks, bounded queues, supervision,
admission proofs, and immutable evidence. They are not callbacks, futures,
ambient RPC, transport protocols, ROS APIs, host implementation kinds, or
privilege paths.

The editable `.panel` source, specialized composite descriptor, resolved plan,
provider observation, run evidence, and Patchbay presentation retain separate
identities. Importing either composite establishes meaning only. It neither
discovers a provider nor grants admission, placement, device, safety, cleanup,
or cancellation authority.

## Correlated request/reply

A specialization declares independent request, reply, and domain-error type
contracts. A request carries a nonzero subject, attempt, correlation, and
idempotency identity. Exactly one terminal semantic outcome may be recorded:

- `reply` for a successful domain reply;
- `domain-error` for a typed negative domain result;
- `timed-out` when the declared monotonic deadline expires; or
- `cancelled` after explicit cancellation.

Transport or provider failure is execution evidence and is never rewritten as
a domain error. Retrying preserves subject, correlation, and idempotency while
creating a new positive attempt identity. Duplicate idempotency submissions
replay the retained state or terminal outcome and perform no new work.

The exact plan retains the request/reply/domain-error types, clock,
correlation, cancellation, and idempotency descriptors plus finite bounds for
in-flight requests, request/reply/domain-error bytes, deadline ticks, retries, replayed
outcomes, timers, evidence, and work per scheduler step.

## Cancellable action

An action specialization declares four independent domain types: goal,
feedback, result, and domain failure. A goal moves through this finite shape:

```text
goal -> queued -> accepted -> feedback* -> result | failed | cancelled
              \-> rejected
       \-> withdrawn-before-admission
accepted | queued -> deadline-exhausted
accepted -> discontinued | provider-lost failure | compatible checkpoint handoff
```

The outcomes are deliberately distinct:

- `rejected` is a pre-work admission decision;
- `withdrawn-before-admission` is cancellation of queued work;
- `cancelled` is admitted work terminated by a causal cancel request;
- `failed` is a running domain or declared provider failure;
- `result` is successful domain completion;
- `deadline-exhausted` is a finite time limit; and
- `discontinued` records an explicit non-handoff plan transition.

No success boolean replaces these typed terminal alternatives. Feedback is
non-authoritative observation. Its payload is not a durable result and the
reference evidence records only bounded byte/count/pressure facts. A slow
consumer follows exactly one declared policy: block producer, drop oldest, or
coalesce latest. The allocator-free reference retains exact item byte sizes:
drop-oldest removes only the oldest items required to admit the new item,
while coalesce-latest replaces the retained feedback set with the newest
item. Both decisions record the disposition and affected-item count without
recording a feedback payload. The portable reference accepts no more than 32
retained feedback items per goal.

## Admission, workload, placement, and cleanup

Before acceptance, the executor checks exact descriptor proofs for:

1. admission authority;
2. workload admission;
3. placement;
4. resource, commit-point, and cleanup policy; and
5. an optional inhibit/safety contract.

The goal is data, never authority. Missing or mismatched proofs reject before
work. The composite also enforces finite concurrent-goal and admission-queue
limits. A robotics goal denied by authority or an active inhibit therefore
produces a visible rejection and cannot authorize motion.

Cancellation records the request event before its terminal consequence and
links the latter to the former. Cleanup follows the already selected exact
resource/commit/cleanup descriptor; cancellation never invents a new cleanup
mechanism or expands authority.

## Retry, idempotency, deadline, and replay

Every retry increments only the attempt identity and is bounded per goal.
The subject, correlation, idempotency, exact plan bindings, authority, and
commit policy remain fixed. Retained duplicate outcomes are finite and evicted
deterministically. Queue exhaustion and retry, cancellation, timer, evidence,
or work exhaustion fail visibly; none creates an unbounded fallback queue.
Timer capacity covers every simultaneously queued or accepted deadline, and
one scheduler advance processes no more than `maximum-work-per-step` expired
subjects before yielding deterministic remaining work to the next advance.

Deadlines use an exact injected clock descriptor. They do not use wall-clock
callbacks. Deadline expiry is terminal and discards retained feedback under
the declared cleanup policy.

## Provider loss and plan transition

Provider loss or a plan transition follows exactly one plan-visible policy:

- terminal failure;
- explicit discontinuity; or
- compatible checkpoint handoff.

Handoff requires both the exact transition proof and the exact checkpoint
descriptor proof. Without either, the action fails closed. A different
provider, placement, authority, resource, cleanup policy, cancellation contract, or incompatible
checkpoint cannot silently resume the same attempt. Evidence records the
selected outcome and preserves goal, attempt, correlation, and idempotency.

## Catalog, plans, evidence, and presentation

`conduit_std::control::STANDARD_CONTROL_CATALOG` is the allocator-free typed
catalog. It publishes the independent type roles and the complete set of fields
which a resolver must specialize into an exact plan. The Rust reference
machines are deterministic implementations of those definitions; they do not
change catalog meaning or claim that a host provider is installed.

The language-neutral contract package
`contract-packages/conduit-std-control.json` exports the same two composites.
`conformance/c4/bounded-control.json` owns cross-host result vectors for Rust,
browser-WASM, process, Python, and allocator-free firmware implementations.
Hosts may report the semantic import as available while honestly reporting no
executable provider.

Patchbay presentation exposes the specialized domain types, all descriptor
identities, every queue/concurrency/feedback/replay/deadline/retry/cancellation/
timer/evidence/work bound, current state, terminal distinction, pressure
decision, and causal identity. Presentation does not fabricate feedback
payload retention, admission authority, provider availability, or handoff.
`conduit_patchbay::project_request_reply` and
`conduit_patchbay::project_cancellable_action` rebuild that view only from an
exact source, plan, run, evidence stream, specialized contract, reference
snapshot, and immutable evidence. A missing provider observation remains
absent rather than becoming a presentation-owned readiness claim.

## Conformance and examples

The canonical fixture matrix includes successful reply and action paths,
several feedback items, domain failure, pre-work rejection, timeout,
cancellation before and after admission, duplicate idempotent submissions,
resource exhaustion, all feedback-pressure policies, concurrency and queue
saturation, provider loss, discontinuity, compatible checkpoint handoff,
unavailable host execution, and robotics authority/inhibit denial.

Tour lesson `library.bounded-control-composites` presents both standalone and
composition stories with a keyboard-readable textual timeline derived from
the reference evidence. Its browser source remains contract-only and fails
resolution when no specialized provider observation exists; it never runs a
literal teaching substitute. Exact hashes and inventory counts remain in
their Rust/conformance owners rather than browser assertions.

## Diagnostics

| Code | Meaning |
|---|---|
| `CND-CTL-001` | unsupported current schema marker |
| `CND-CTL-002` | invalid subject/attempt/correlation/idempotency identity |
| `CND-CTL-003` | missing or invalid type/policy descriptor |
| `CND-CTL-004` | a required finite bound is zero or absent |
| `CND-CTL-005` | specialization exceeds the portable reference ceiling |
| `CND-CTL-006` | identity reuses a subject, correlation, or idempotency inconsistently |
| `CND-CTL-007` | transition is illegal from the current state |
| `CND-CTL-008` | request or goal exceeds its byte bound |
| `CND-CTL-009` | reply exceeds its exact byte bound |
| `CND-CTL-010` | feedback exceeds its per-goal byte bound |
| `CND-CTL-011` | deadline is expired or outside the plan bound |
| `CND-CTL-012` | retry bound is exhausted |
| `CND-CTL-013` | cancellation bound is exhausted |
| `CND-CTL-014` | immutable evidence capacity is exhausted |
| `CND-CTL-015` | transition/checkpoint handoff proof is absent or incompatible |
| `CND-CTL-016` | action result exceeds its exact byte bound |
| `CND-CTL-017` | domain failure or request/reply domain error exceeds its exact byte bound |

## Normative requirements

| ID | Obligation |
|---|---|
| `CTL-001` | Keep the composites above core and allocator-free in the reference implementation |
| `CTL-002` | Preserve independent domain request/reply/error and goal/feedback/result/failure types |
| `CTL-003` | Preserve nonzero subject, attempt, correlation, and idempotency identities |
| `CTL-004` | Admit action work only after exact authority, workload, placement, resource/commit/cleanup, and optional inhibit proofs |
| `CTL-005` | Bound every queue, concurrency set, feedback buffer, replay set, deadline, retry, cancellation, timer, evidence store, and step |
| `CTL-006` | Keep rejection, withdrawal, cancellation, failure, result, deadline exhaustion, and discontinuity distinct |
| `CTL-007` | Keep feedback non-authoritative and omit its payload from durable evidence |
| `CTL-008` | Require explicit compatible proofs for checkpoint handoff and fail closed otherwise |
| `CTL-009` | Keep catalog meaning, provider observation, exact plan, evidence, and Patchbay presentation distinct |
| `CTL-010` | Publish one current contract-package form, conformance matrix, deterministic reference, and accessible Tour lesson |
