# Optional managed-component lifecycle facet

Status: candidate normative C4 contract. The fixture
`conformance/c4/managed-component-lifecycle.json` exercises MCL-001 through
MCL-012.

## Scope

A managed component is one exact resolved implementation instance in one run,
plan epoch, and activation generation. The facet is optional. Stateful,
resource-owning, long-lived, or effectful implementations may offer it; pure
stateless transforms and ordinary finite invocations do not acquire a wrapper.

The facet is an interface on the generic implementation manifest from
specification 079. Native, WASM, supervised-process, firmware/FFI, remote, and
deterministic adapters use the same descriptor, request, observation, reason,
and evidence forms. An adapter advertises only the facets it can prove. Audio,
HTTP, and application sockets do not own domain-private lifecycle registries.

## Exact identities

An observation retains the component and semantic contract; implementation
ID, version, semantic identity, and complete artifact set; host, boot, and host
observation; run and plan identity; plan epoch and activation generation; and
the exact resource bindings, grants, and leases. Host boot identity is a
canonical host-report and sealed-plan fact, not an alias for the report ID.

The implementation manifest pins both the shared managed-component protocol
interface and the exact provider lifecycle-profile descriptor as provided
interfaces. Compile documents preserve required and provided interfaces when
they reconstruct or hash a manifest. Installation, discovery, provider
availability, and authority remain separate facts.

Selection follows the existing implicit-satisfaction contract rather than
comparing interface labels. The complete implementation-role proof keeps the
semantic contract as its required operand and the exact implementation
manifest as its offered operand. Its `lifecycle` obligation binds the shared
protocol requirement to the offered provider-profile hash, while namespaced
structural facets prove the requested prepare, activate, quiesce, retained
state, cleanup, bounded-cancellation, and progress behavior. Native and remote
implementations can therefore satisfy the same requirement with different
profile and proof identities. Behavioral conformance remains a third,
separate fact; a manifest declaration or structural proof does not claim that
fixtures passed.

## States and independent dimensions

The component states are `configured`, `prepared`, `active`, `quiescing`,
`inactive`, `cleaning`, `stopped`, `failed`, and `unsupported`.

The following are independent and MUST NOT be folded into that state:

- runtime readiness (`ready`, `waiting`, or not applicable);
- work obligations, attempts, checkpoints, retries, and receipts;
- whole-run Start, Active, Waiting, Drain, Abort, and Terminal;
- candidate-plan preparation, handoff, commit, rollback, and retirement;
- provider availability, host/app/OS lifecycle, and host loss;
- grants, leases, resource conflicts, and independent inhibit state.

In particular an HTTP listener may be `active` while scheduler readiness is
`waiting`. A test obligation and a CI shard are not managed components merely
because they use a managed Tour server.

## Requests and commits

Prepare, activate, quiesce, deactivate, clean, and stop requests name the
expected plan epoch, activation generation, and observation sequence. They
carry an external authority identity and a finite deadline. The request
context reports implementation availability, grant state, resource admission,
lease freshness, and inhibit independently.

Acceptance records intent only. It does not prove a transition. Provider
events establish explicit preparation, activation, admission-closed,
quiescence, cleanup-started, and cleanup-complete commit points. Progress is
bounded evidence and never completion. Duplicate request IDs are idempotent;
stale IDs, epochs, generations, observations, host facts, grants, resources,
and leases fail distinctly.

An admitted provider commit or current scheduler readiness observation renews
the component observation only for the descriptor's finite request horizon.
It does not rewrite the pinned host/boot observation or grant a new lease. A
configured component with no newer provider/runtime observation still expires
and rejects activation from stale facts.

## Production executor seam

The completed hosted-provider path remains authoritative:

`Handler::prepare -> Handler::start -> bounded Handler::step/wake ->
Handler::cancel -> bounded Handler::cleanup`.

The executor projects those existing commits into the optional lifecycle
machine. It never invokes a second provider adapter and never infers the facet
from a process, PID, provider name, socket reachability, or UI control. The
Linux HTTP listener, observed ALSA capture/playback, and bounded application
socket providers offer the same generic manifest interface.

Drain closes new admission before in-flight disposition. Abort and natural
completion still use the existing cleanup callback and exact cancellation
deadline. A cleanup timeout or failure produces `failed` plus an independent
cleanup disposition. Provider loss requires cleanup; host loss may make
cleanup unprovable. Neither loss implies inactive, clean, or stopped.

## Evidence and inspection

Evidence retention and progress counts are finite descriptor bounds. A
bounded read detects an expired cursor. The typed inspection projection keeps
state, reason and code, generation, resources, grants, leases, readiness,
cleanup, freshness, and retirement separate. Presentation may request a
transition but cannot claim its commit or collapse these dimensions to a
green/red boolean.

## Normalized reasons

`CND-MCL-000` through `CND-MCL-031` identify normal commits and the required
rejection/failure categories. `CND-MCL-032` through `CND-MCL-034` identify
descriptor and component-identity failures. `CND-MCL-035` identifies
activation failure. Exact meanings are defined by `ManagedLifecycleReason`
and exercised by the current conformance fixture.

## Requirements

| ID | Requirement |
|---|---|
| MCL-001 | Keep the facet optional and tied to one exact generic implementation instance |
| MCL-002 | Preserve component, implementation/artifact, host/boot, run, plan epoch, generation, resource, grant, and lease identity |
| MCL-003 | Keep readiness, work, run, plan transition, host/provider, authority, and inhibit dimensions separate |
| MCL-004 | Fence authorized requests by exact component, epoch, generation, observation, freshness, and finite deadline |
| MCL-005 | Treat request acceptance and bounded progress as non-commit evidence |
| MCL-006 | Require explicit provider commit points for preparation, activation, admission closure, quiescence, cleanup, and stopped |
| MCL-007 | Preserve the single hosted-provider execution path and project its existing callbacks |
| MCL-008 | Let every adapter boundary claim only explicit provable facets |
| MCL-009 | Never infer clean, inactive, or stopped from failure, provider loss, or host loss |
| MCL-010 | Retain finite normalized evidence and distinct stable reason codes |
| MCL-011 | Publish a typed non-authoritative inspection projection without boolean status collapse |
| MCL-012 | Prove standing HTTP, leased audio, network, and CI-composition boundaries with current fixtures |
