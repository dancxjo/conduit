# Portable execution arrangements

Conduit's logical plan does not become a thread graph, process graph, CPU
graph, or host graph when it executes. Physical execution adds a separate,
exact arrangement beneath the same logical nodes and cords:

```text
exact logical plan
  -> execution regions
    -> placements
      -> finite lanes
        -> bounded proposed results
          -> deterministic commit domains
```

A **placement** is an independently owned authority, resource, lifecycle, and
failure-containment boundary. A hosted process, WASI instance, remote Conduit
host, or ConduitOS protection domain may realize a placement, but none of
those mechanisms is portable semantics.

A **lane** is one finite physical execution resource within a placement. A
provider reports whether the lane has independently progressing execution,
simultaneous execution, preemption, termination, and isolation as
`unsupported`, `observed`, or `guaranteed`. Admission that depends on a
behavior requires `guaranteed`; a measurement cannot become a promise.

An **execution region** is the smallest physical scheduling and placement
unit. It names its logical members but does not replace their identity. The
compiler may separate only structures for which independence is proven.
Effectful, state-coupled, feedback, coupled fan-out, shared-resource,
sensitive, hazardous, ordered, and ambiguous-merge work stays together or is
serialized until an exact contract proves safe separation.

Every logical cord that crosses regions has exactly one finite **boundary
realization**. It retains the logical cord's item and byte capacities. An
in-address-space queue, cross-lane mailbox, bounded IPC pair, or distributed
cord provider changes the physical realization, not the cord's value,
ordering, pressure, cancellation, failure, terminal, or evidence semantics.
No extra callback or transport queue exists outside admission.

## Compute and commit

Lanes compute concurrently only after all lanes, ready and wake slots,
boundary storage, scratch, timers, proposal slots, commit windows,
cancellation disposal, and evidence storage have been reserved before
`Start`.

A lane never mutates authoritative cord or lifecycle state. One bounded step
returns one proposed result containing staged consumption, publication,
resource usage, terminal/error outcome, and physical observations. The
runtime validates the proposal against the current plan epoch, authority,
resources, cancellation state, and plan-derived ticket or domain key. It then
commits or disposes the proposal exactly once.

Physical start and finish order is retained as provider evidence. It never
chooses semantic value order, effect order, terminal outcome, or normative
evidence order. External effects occur only at an exact commit point.

The single-lane deterministic executor remains the oracle. Parallel providers
must normalize to the same logical results under reversed and adversarial
physical completion orders.

## Current compile and admission path

The current compile-input form requires independently observed execution
placements and lanes in every host report, plus explicit proposal, commit,
cancellation, evidence, boundary-realization, and plan-epoch policy. The
resolver retains the selected observations in its own exact identity. The
compiler then emits a separately identified execution arrangement alongside
the logical plan; the logical plan identity and shape do not absorb physical
regions, lanes, or provider mechanisms.

`conduct` and the browser run path reject a missing, altered, or wrong-epoch
arrangement before exact-run admission. The hosted fixed-lane provider proves
bounded simultaneous proposal computation and deterministic proposal order in
its conformance path. The ordinary hosted scheduler still commits through the
single-lane oracle while its node workspaces are being connected to those
lanes. Until that connection is complete, the repository does not claim that
ordinary hosted plan execution is physically simultaneous.

## Placement isolation profiles

Providers advertise only the strongest complete profile they prove:

- `step-native`: trusted bounded nonblocking steps run directly on a lane;
- `isolated-cooperative`: contained work yields bounded proposals;
- `isolated-preemptible`: Conduit regains control within a finite admitted
  bound;
- `isolated-terminable`: Conduit can fence further effects, stop execution,
  and reclaim every resource declared reclaimable.

A timeout is not termination. A cancellation request, regained control,
effect fencing, stopped execution, and reclaimed resources are separate
observations. Loading an implementation never grants it authority.

## Constrained firmware generation

`conduit-embedded-build` is the hosted boundary between an exact checked plan
and the allocator-free `conduit-embedded` executor. A firmware build supplies
the exact policy package and lock hashes, the full Conduit revision, and one
driver/port-ordinal binding for each planned instance. The generator validates
the complete `ExecutionPlan`, seals those identities with the embedded profile,
and emits one fixed Rust module containing the node, driver, host-operation,
port, queue, and storage-facing plan data. Firmware includes that module; it
does not parse source or resolve providers on the device.

Each driver host-operation ordinal is build input, not firmware-owned meaning.
Generation resolves it to one required effect and resource from the checked
plan and emits the semantic action, concrete resource, effect and grant
identities, lease and commit-profile identities, capability and grant IDs,
selected host, and use-time-check requirement. The executor refuses an ordinal
that is absent from that node's generated bindings before calling the host.
Host adapters dispatch on the generated semantic action and receive the exact
binding plus run and tick attribution; private numeric switch tables are not a
second authority or resource model.

Lowering fails closed when the constrained executor would approximate a plan.
The current supported subset requires hard-bounded implementations, bounded
cancellation, enforced step limits, exact fixed queue bytes, full-capacity FIFO
blocking pressure, and one cord per port. Ordinary exact authority bindings
with finite resource leases and effect commit profiles are supported. Policy
budget authority, constraint-bearing authority, administrative containment,
and required resources that are not owned by a generated host operation remain
unsupported, as do hazard closure, distributed cords, fan-out, merge,
supervision, pools, runtime-evidence projection, and the other explicitly
rejected features. Each must gain production embedded semantics before a plan
containing it can be generated. In particular, this generator does not yet make
a hazardous robotics plan executable.

Every embedded driver exposes its exact descriptor. The executor compares the
ordered runtime driver set with the generated bindings before prepare or any
host effect. The generated-representation identity is separate from the full
logical plan hash and is returned by preflight alongside the selected embedded
profile.

## Ownership

The portable runtime owns region readiness, deterministic selection and
commit, cancellation policy, proposal disposition, and logical evidence.

The placement owns its admitted authority/resource boundary, lifecycle,
generation, containment, and failure behavior.

The lane provider owns physical start, park, wake, resume, interruption, and
provider observations. Hosted worker APIs, remote transport, CPU/AP identity,
interrupts, page tables, and resumable contexts stay in their provider
implementations.

Remote placements therefore run their own admitted local lanes and scheduler;
they are not per-step RPC workers behind a permanent central scheduler.
Distributed cords carry already committed values using their own bounded,
generation-fenced delivery contract. A host loss or placement move is an
explicit exact-plan transition, never opportunistic rescheduling.

ConduitOS follows the same nesting. CPUs realize lanes; interprocessor
interrupts realize bounded wakes; optional address spaces and resumable
contexts realize isolation profiles. CPU identity and timing never become
logical order.

## Proof boundary

Portable conformance requires:

- one-, two-, four-, and finite-N lane admission;
- three independent regions entering computation before any finishes;
- adversarial completion with identical deterministic commits;
- same-lane and cross-lane wake, pressure, mailbox saturation, and stale
  generation cases;
- cancellation, provider loss, stale epoch, terminal races, and proposal
  disposal exactly once;
- honest preemption, termination, fault containment, and resource
  reclamation;
- normalized fixtures through materially different simultaneous providers and
  materially different isolation providers.

The hosted fixed-lane provider and ConduitOS multicore provider are the first
simultaneous pair. The WASI/Wasmtime placement and ConduitOS protection-domain
provider are the first isolation pair. Software checks may prove the portable
contract; they do not claim unobserved bare-metal behavior.
