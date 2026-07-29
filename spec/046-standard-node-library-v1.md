# Standard node library contract version 1

Status: proposed portable contract

Depends on: specifications 007, 008, 010, 012, 022, 024, 039, 041, 042,
043, 044, 045, and the common supervision contract in 049

## Boundary

Standard libraries make common mechanics pleasant without making them magic.
Literals, transforms, filters, folds, finite windows, debounce, throttle,
delay, bounded retry, and probes are ordinary typed nodes. Their port,
configuration, terminal, cancellation, storage, timer, work, and evidence
facts remain visible in the resolved plan. Discovery of an implementation does
not make it usable.

This contract does not decide who may create or delegate authority, install a
provider, enroll a realm, activate an artifact, admit a hazardous effect, or
clear an independent inhibit. Those decisions remain owned by the safety
contracts. A standard node consumes the exact plan-supplied facts or fails.

## Common bounded shape

Every standard node declares positive per-step work and evidence bounds.
Stateful nodes declare finite retained value and byte limits. Time-shaping
nodes declare finite timer capacity in an explicit plan-selected time basis.
Hosted operations additionally declare finite pending-operation and request
and response byte limits. Zero is a valid declaration only when the resource
is not used by that node family; it never means unlimited.

Terminal and cancellation policies are named semantic inputs. Executor wake
order, registry order, wall-clock order, and provider discovery order cannot
select behavior. Checkpoint data contains node state only: grant, provider,
resource, and administrative authority must be rebound from a newly validated
plan and are never restored as node state.

## Retry and supervision

Retry and the standard supervisor consume `TerminalObservation` and emit only
the finite `SupervisionDecision` vocabulary from specification 049. Retry pins
a positive maximum attempt count, deadline, finite backoff, exact provider
binding, resource binding, grant, cancellation scope, and enough evidence
capacity for every attempt. Every attempt retains those identities. Retry
cannot discover or select another provider, enlarge a request, mint or
delegate a grant, reset a persistent budget, or change a hazardous admission.

Fallback is an explicitly planned compatible choice. It cannot mean “try a
more permissive provider.” Standard retry, fallback, circuit-breaker,
supervisor, health-gate, terminal-projection, operator-action, and fault-source
nodes do not own parallel error semantics. Supervision may restart
implementation state within its existing plan contract, but cannot treat
restart or checkpoint restore as an authority epoch.

## Narrow host-service interfaces

A host-service contract names one narrow versioned interface and operation.
Each request consumes an externally supplied provider binding, resource
binding, grant, cancellation scope, byte limits, pending limit, and evidence
limit. There is no mega-host interface and no ambient lookup path. Filesystem,
blob/KV, process, secret, cryptographic, raw-network, transport, and HTTP
operations do not double as enrollment, installation, activation, or
administration.

Dangerous services are absent from default and reference registries. A host
capability report states availability and finite limits, not permission to use
the service. Resolution accepts only the exact requested interface version,
operation, and provider with sufficient request, response, pending-operation,
and evidence capacity in a currently valid report. Unsupported, insufficient,
not-yet-valid, and stale capability reports fail before execution.
Administrative and hazardous-use policy remains an external admission step.
An independently validated grant must name the exact request grant; capability
availability never converts denial or a different grant into authority.

## Stable requirements

- STD-001: standard mechanics remain ordinary typed plan-visible nodes.
- STD-002: storage, bytes, pending work, timers, per-step work, and evidence are finite.
- STD-003: terminal and cancellation behavior are explicit semantic inputs.
- STD-004: retry keeps exact provider, resource, grant, and cancellation identities.
- STD-005: retry, fallback, restart, and restore never amplify authority.
- STD-006: checkpoint state never persists or reconstructs authority.
- STD-007: each hosted operation uses a narrow interface and exact external bindings.
- STD-008: discovery and capability reporting never imply usability.
- STD-009: dangerous providers are absent from default/reference registries.
- STD-010: host services never double as installation or administration.
- STD-011: host-service resolution rejects stale, unsupported, or insufficient capabilities.
- STD-012: capability availability remains distinct from exact grant authorization.

The normative fixture is `conformance/c4/standard-node-library-v1.json`.
