# Fresh host reports and deterministic resolution version 1

Status: candidate normative C5 contract. `conformance/c5/host-resolution-v1.json`
exercises `HST-001` through `HST-012` and `RES-001` through `RES-026`.

## Fresh capability reports

A `CapabilityReport` is a canonical, versioned observation with a host,
observation identity, reporter and trust descriptor, named time basis,
observed-at and valid-until ticks, aggregate available resources, semantic
capabilities, concrete resource pools, topology facts, executor/target/ABI
support, plan-version range, and current constraint hashes (`HST-001`).
Collection order is not identity (`HST-002`).

Capabilities, resources, and topology remain distinct (`HST-003`). A
capability pins a semantic interface, mode, subject, capability-specific detail
hash, and finite capacity. A resource pins a concrete resource and its current
capacity/exclusivity. A topology observation pins a domain-owned relationship,
endpoints, reachability, MTU, sessions, and details. This generic form carries
network, browser, embedded, storage, crypto, clock, process, and device facts
without adding those product concepts to `conduit-core` (`HST-004`).

Report validation is allocator-free and bounded by caller scratch (`HST-005`).
It rejects malformed identity, time-basis mismatch, observations from the
future, expiry, and unsupported plan versions with stable `CND-HST-*` reasons
(`HST-006`). Expiry invalidates a plan input and requires explicit
re-resolution; it never silently refreshes or substitutes a host (`HST-007`).
Reporter trust is a resolver policy input and capability never implies
authority (`HST-008`).

Reports say what is available now. They do not request or perform discovery,
installation, login, flashing, scans, association, interface/address/route
changes, certificate issuance, process launch, socket open, or any other host
mutation (`HST-009`). Typed inspection exposes identities, capacity,
capabilities, resources, topology, reporter/trust, and freshness without
refreshing the report (`HST-010`).

Protocol/security combinations and device-specific limitations are expressed
as domain-owned capability/detail/topology descriptors, not one readiness
boolean (`HST-011`). Narrow host interfaces are reported independently; clocks
and HTTP support do not imply filesystem, process, entropy, secret, crypto, raw
network, distributed-cord, or serving support (`HST-012`).

## Resolver inputs and predicates

The hosted resolver consumes only immutable explicit inputs: semantic
placement requests, implementation and artifact manifests, capability reports,
authority decisions, requested finite allocations, capability/resource/
topology predicates, and one resolver policy (`RES-001`). Implementation
manifest pins provide the contract, executor, artifacts, execution profile,
interfaces, authority/effect requirements, and supported versions. Hosted
predicates refine current mode, subject, detail, topology, and capacity
requirements without changing semantic node identity (`RES-002`).

Every implementation manifest, artifact manifest, and report is validated
before selection (`RES-003`). Required artifacts, target, ABI, executor,
plan-version, capability, capacity, resource, topology, report trust, policy,
and authority boundaries all fail closed (`RES-004`). Capability satisfaction
never manufactures a grant (`RES-005`).

Requests and candidates are canonicalized before selection. Discovery,
registry, filesystem, map, and input order cannot affect the result
(`RES-006`). Explicit policy may rank implementation IDs. A policy may either
select the lowest complete canonical identity or reject equally ranked
solutions as ambiguous (`RES-007`). Candidate order is never evidence.

Resolution searches complete placement combinations while reconciling
aggregate per-report, per-capability, and per-resource-pool capacity
(`RES-008`). Candidate enumeration names an exact resource ID when more than
one concrete pool can satisfy a predicate; exclusive resources bind at most
once in a solution. The policy supplies a finite maximum search-state count;
exhaustion fails with `CND-RES-019` rather than producing a partial arrangement
(`RES-009`). This bounded reference search is deterministic, not a claim of
globally optimal scheduling.

Success contains exact instance, semantic contract, implementation identity,
artifact digests, host/report identity and freshness, allocation, capability
subjects, authority grant IDs, resolver identity, and policy hash
(`RES-010`). The ordinary planner retains source topology and constructs the
existing `ExecutionPlan`; `seal_resolved_execution_plan` proves that every
selection and observation was copied exactly, then invokes the portable plan
validator (`RES-011`). There is no second plan type.

Failure retains every statically rejected candidate with a stable ordered set
of reasons and separately records global ambiguity, aggregate capacity, or
search-limit failure (`RES-012`). The tree contains no protected values.

Linux and RP2040 hosts can satisfy the same semantic capability through
different executors, subjects, artifacts, resource bounds, and reports
(`RES-013`). Multiple instances may be placed across reports only after
aggregate capacity reconciliation (`RES-014`).

Structural capability satisfaction retains the directional proof pins required
by specification 027; shape, language, ABI, or candidate order alone never
establishes compatibility (`RES-015`). A future replacement resolver consumes
the same explicit facts and constructs a new exact plan epoch. It does not edit
the active plan (`RES-016`). Overlap, exclusive resources, state contract,
handoff, rollback, security/delivery floors, and every optional weakening must
be explicit inputs (`RES-017`).

Network predicates can independently pin address family, bind/connect/listen,
interface/endpoint, MTU/buffers, distributed backend/mode/reachability,
HTTP/version/upgrade, TLS/mTLS or trusted-proxy boundary, opaque certificate or
trust-store handles, privileged-port authority, and finite listener,
connection, session, and body limits (`RES-018`).

Wi-Fi predicates independently describe interface ownership/freshness; AP,
station, monitor, and concurrency modes; radio/regulatory facts;
authentication/cipher/key management; finite station/interface/packet/socket/
lease/DNS/route/neighbor/NAT/timer/retry pools; attachment/uplink topology;
interference/exclusivity; and cleanup/restoration capability (`RES-019`).
Association, DHCP, addressing, DNS, routing, bridge/NAT, raw connect/listen,
and local services are separate capabilities (`RES-020`).

The resolver observes these facts but performs none of the effects represented
by them (`RES-021`). It does not fetch artifacts, acquire grants, retrieve
secret bytes, or weaken HTTPS to HTTP (`RES-022`). Private keys and credentials
remain opaque host handles (`RES-023`). “Ready” is always a constraint
evaluated against current evidence, never a persistent implementation property
(`RES-024`).

`ResolvedPlacement` is presentation-neutral and its deterministic
`search_states` count is diagnostic, not semantic or performance evidence
(`RES-025`). Wall-clock duration and ambient runtime state are not resolver
inputs (`RES-026`).

## Stable reasons

- `CND-HST-001` unsupported report schema
- `CND-HST-002` stale report
- `CND-HST-003` malformed report
- `CND-HST-004` report identity mismatch
- `CND-HST-005` time-basis mismatch
- `CND-HST-006` observation is from the future
- `CND-HST-007` unsupported plan version
- `CND-RES-001` through `CND-RES-019` candidate/search reasons defined by
  `CandidateRejectionReason`
- `CND-RES-020` through `CND-RES-026` exact-plan sealing reasons defined by
  `PlanSealingReason`
