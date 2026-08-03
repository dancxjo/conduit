# Audited robotics profile and command policy current form

Status: candidate normative C5 contract. The fixture
`conformance/c5/netherwick-describe-only-profile.json` exercises the first
Netherwick profile without hardware.

Current source anchor: `dancxjo/netherwick@f43ff13846b47b05e133d0321bdbaafffd1bcdbe`
(Netherwick PR #140).

## One semantic profile, ordinary implementations

`conduit.robotics/profile` describes a bounded robotics boundary and carries
pure command-policy semantics. It is not a robot runtime, driver, discovery
service, resolver, simulator, scheduler, synchronization primitive, or grant.
Linux and Pico W candidates publish the same contract through the ordinary
generic host-conformance model. Their implementation, artifact,
provider-bundle, adapter, build, target, and host facts remain distinct.

The profile lives in `conduit-robotics`, outside allocator-free
`conduit-core`. Core retains only domain-neutral descriptor, implementation,
host-observation, binding, authority, plan, and evidence mechanisms. No
robotics registry or resolution path exists.

## Distinct domain values

Observation, command, acknowledgement, safe outcome, possession, terminal,
and fault have independent exact type identities. A role, membership,
capability, network attachment, discovered endpoint, or possession lease is
data and never substitutes for motion authority. Command acceptance is not a
safe physical outcome, and a terminal outcome is not a fault payload.

Every physical quantity declares exact units, frame, time basis, uncertainty
bound, and maximum age. The Pete profile currently covers linear and angular
velocity, distance, heading, acceleration, voltage, current, and charge.
Missing or mismatched metadata is a typed failure; no ambient transform,
clock, unit conversion, or freshness policy is inferred.

## Finite motion admission

The profile pins a finite command TTL, linear and angular velocity envelope,
current-possession requirement, motion authority, bounded stop, emergency
stop, not-charging condition, charging interlock, clear safety inhibit, and
exact motion capability. All must be satisfied independently before motion can
be admitted. Validation and policy decisions remain effect-free and never
perform synchronization, dispatch, or actuation.

## Portable command flow

The exact `conduit.robotics/command-flow/two-lane-bounded-execution` policy
separates a single ordinary ingress slot, a single latest-motion ingress slot,
and the sixteen-entry downstream execution queue. No one-slot value is
presented as a bound for the complete command flow.

The checked program classifies a command as ordinary, replace-same-kind,
latest-motion, stop, or emergency stop. A pure allocator-free transition reads
the complete two-slot state and returns the exact next state plus at most two
accepted command identities whose lifecycles must be interrupted. Ordinary
pressure rejects busy. A replace-same-kind refresh may replace only an exact
kind match. Latest motion uses an exact wrapping `u32` half-range comparison;
an active-motion renewal owns no separately interruptible queued lifecycle.
Stop and emergency stop clear both ingress lanes before occupying the ordinary
lane, and a later motion command begins a new sequence epoch.

The host chooses atomics, locks, interrupts, tasks, threads, wakes, and queue
storage. The allocator-free execution queue retains opaque host payloads while
Conduit owns the configured arbitration: ordinary work rejects full before
mutation; latest motion removes queued motion, renews an equal active motion
without dispatch, or interrupts changed active motion and runs first; latest
safety recovery replaces queued recovery; and stop or emergency stop returns
every distinct queued lifecycle, interrupts active work, clears the queue, and
runs first. Active implementation preemption is reported separately from
lifecycle interruption so a same-identity replacement still stops the old
action without falsely closing its lifecycle. No rejected transition mutates
queue state.

The independent physical safety floor supplies an explicit active-recovery
observation. While it is active, program commands cannot supersede it; stop and
emergency stop remain the only portable preemption. The host must apply each
transition while holding its chosen synchronization boundary, close every
named interrupted lifecycle, and perform any returned active deadline renewal
or dispatch. Physical hazard detection and response, device access, clocks,
and motor writes remain outside this policy and outside the checked program.

## Static description versus current observation

Describe-only reports use `HostClass::DescribeOnly` and
`HostExecutionMode::DescribeOnly`. They may publish linked compiled providers,
carrier candidates, logical relationships, role descriptors, and checkpoint
formats. They contain no boot, possession, authority, initialized-provider, or
path observation. A generic binding attempt therefore fails with
`CND-HCF-003` before execution.

Stable logical relationships are separate from observed carrier paths. A path
observation, when produced later by an executable host, must carry its provider
identity, generation, time basis, validity interval, limits, and sensitivity.
Description cannot join a network, relay traffic, enroll an entity, possess a
body, promote a role, load a checkpoint, activate a plan, open a device, or
actuate.

## Inspection and redaction

A host report publishes descriptor pins and bounded public facts only. Raw
device handles, credentials, bearer tokens, private endpoints, and sensitive
topology are forbidden. Entity, boot, role, possession, and authority are
separate optional facts; absence remains visible rather than being filled from
discovery.

Tour lesson `platform.audited-robotics-profile` uses the existing generic
provider-matrix and ordered textual evidence presentation. Its representative
real panel is executable on the normal provider-bearing host and is rejected
unchanged by each describe-only profile. The lesson adds no teaching runtime
or presentation-owned authority.

## Stable outcomes

- `CND-RBT-001`: unsupported profile schema
- `CND-RBT-002`: malformed descriptor or identity mismatch
- `CND-RBT-003`: value-role mismatch
- `CND-RBT-004`: units, frame, clock, envelope, or uncertainty mismatch
- `CND-RBT-005`: stale observation
- `CND-RBT-006`: expired or overlong command TTL
- `CND-RBT-007`: possession or motion authority absent or confused with data
- `CND-RBT-008`: stop, e-stop, charging interlock, or inhibit requirement absent
- `CND-RBT-009`: discovery or description presented as enrollment/live state
- `CND-RBT-010`: hidden device handle
- `CND-RBT-011`: unsupported or malformed host profile
- `CND-RBT-012`: secret or sensitive topology disclosure
- `CND-RBT-013`: describe operation caused an effect
- `CND-RBT-014`: required capability absent
- `CND-RBT-015`: ordinary ingress is busy
- `CND-RBT-016`: motion sequence is stale
- `CND-RBT-017`: execution queue is full
- `CND-RBT-018`: independent physical safety recovery is active

## Requirements

| ID | Requirement |
|---|---|
| RBT-001 | Keep robotics semantics outside core and reuse the generic implementation, host, binding, plan, and evidence path |
| RBT-002 | Keep observation, command, acknowledgement, safe outcome, possession, terminal, and fault identities distinct |
| RBT-003 | Pin units, frame, clock, uncertainty, and freshness for every physical quantity |
| RBT-004 | Require finite TTL, velocity envelope, distinct ingress and execution-queue bounds, authority, capability, stop, e-stop, charging/interlock, and inhibit facts |
| RBT-005 | Let materially different hosts implement one contract with distinct exact manifests and host facts |
| RBT-006 | Keep compiled inventory, current initialization, logical relationships, and observed paths distinct |
| RBT-007 | Make describe and check effect-free and incapable of enrollment, possession, promotion, activation, or actuation |
| RBT-008 | Reject hidden handles and secret or sensitive topology in inspectable reports |
| RBT-009 | Decide two-lane ingress and bounded execution arbitration with deterministic allocator-free transitions while the host owns synchronization, scheduling, dispatch, actuation, and the independent physical safety floor |
