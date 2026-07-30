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

## Canonical library paths

Standard contracts use domain-first canonical paths. The path describes the
problem domain, not the implementation mechanism:

- `std/literal`, `std/format`, and the fundamental `std/...` types;
- `flow/identity`, `flow/merge`, and the other structural operators;
- `time/delay`, `state/cell`, and `supervision/retry`;
- `fs/read`, `process/run`, and `net/http/serve`.

Node and type identities occupy distinct grammar and catalog positions, so
their paths do not repeat `node` or `type`. There is no compatibility rewrite
from the former flat `conduit.std/...` spelling. A resolved plan retains the
same canonical path that source selected.

`host` is reserved for the execution environment and its observations.
Consequently the HTTP server operation is `net/http/serve`, not
`net/http/host`.

## Standard type universe

The standard type catalog publishes meanings independently of representations:

- fundamentals: `std/unit`, `std/bool`, `std/integer`, `std/natural`,
  `std/float`, `std/decimal`, `std/text`, and `std/bytes`;
- fixed-width integers: `std/i8` through `std/i128` and `std/u8` through
  `std/u128`;
- structural constructors: `std/option`, `std/result`, `std/list`, `std/map`,
  and `std/reference`, plus `std/record` and `std/variant`;
- time, identity, and operations: `std/duration`, `std/instant`,
  `std/timestamp`, `std/id`, `std/error`, `std/terminal`, `std/health`, and
  `std/progress`;
- domain types such as `net/ip/address`, `net/http/request`, `fs/path`,
  `process/exit-status`, and `crypto/digest`.

`std/integer` is the mathematical signed integer and `std/natural` is the
mathematical nonnegative integer. Fixed-width contracts are used when range,
overflow, layout, serialization, registers, or FFI are semantic. A host that
supports only 64-bit values may still discover `std/integer`; it separately
advertises a finite representation limit such as 64 integer bits.

Generic catalog entries are constructors, not usable unspecialized
`TypeContractRef` values. A concrete specialization has an exact descriptor
and semantic hash. The initial catalog publishes `std/list/text` as the
concrete finite text-list specialization used by Panel configuration.

For any standard contract, resolution keeps four questions separate:

1. **defined**: the semantic catalog knows the contract;
2. **provided**: a selected host advertises an exact implementation or
   representation with sufficient finite limits;
3. **authorized**: the plan has the required grants for its effects; and
4. **placeable**: the resolver can assign it to an eligible realm member.

A negative answer after the first question is an availability, authority, or
placement failure. It never makes a known standard contract invalid syntax.

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
- STD-013: canonical library identities are domain-first paths with no legacy rewrite.
- STD-014: type definition and host representation support remain separate facts.
- STD-015: mathematical and fixed-width integer meanings remain distinct.
- STD-016: generic constructors are not concrete type references without arguments.
- STD-017: polymorphic node definitions publish type-parameter relationships and
  are specialized to exact type-contract references before plan emission.

## Concrete catalog publication

`conduit-std` publishes the version-one catalog above `conduit-core`. Each
entry contains an ordinary typed `NodeContract`, typed
configuration, explicit ordering/terminal/cancellation/pressure policy
identities, finite resource ceilings, an exact reference-provider identity,
and any narrow host-service requirement. It contains no registry, executor,
host framework, ambient lookup, or domain concept.

Polymorphic entries additionally publish an authoritative generic signature.
The signature relates port indexes to parameters or applications such as
`std/option<T>`; the ordinary `NodeContract` is only the bounded concrete
specialization exercised by allocator-free reference fixtures. A compiler must
specialize the generic signature to exact `TypeContractRef` values before it
emits a plan. In particular, bytes in a reference specialization never mean
"any value."

The catalog currently publishes source/sink, structural, transformation,
time, state, supervision, testing, boundary, and independently composable
network contracts. `conformance/c4/standard-catalog-v1.json` maps every
published contract to stable requirement IDs and the five required fixture
classes. The allocator-free conformance runner exercises those classes across
deterministic, hosted, and honest constrained/unsupported profiles and
compares provider-independent normalized evidence. Boundary-specific effect
fixtures and value-semantic reference implementations remain separate from
this catalog-level proof.

The normative fixtures are:

- `conformance/c4/standard-node-library-v1.json`
- `conformance/c4/standard-catalog-v1.json`
