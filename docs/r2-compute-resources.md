# R2 compute resources

Compute is an ordinary finite resource in the existing host-advertisement,
planning, reservation, and kernel-preparation path. It is not a second
allocator, scheduler, planner, or execution API.

## Contract boundaries

- A host advertises architecture-neutral compute pools. Physical packages,
  cores, hardware threads, and base scheduler identifiers remain base
  facts rather than authored form facts.
- A capability requirement states minimum, preferred, and maximum lanes, a
  minimum service guarantee (`Shared`, `Reserved`, or `Exclusive`), and optional
  topology constraints. The planner admits every minimum before assigning spare
  capacity toward preferences.
- A sealed Plan records the selected lane count, pool, service guarantee,
  architecture-base identity and kind, and any selected stable topology
  group. Current utilization and concrete lane assignment are separate runtime
  facts.
- A transient lane assignment belongs to an active play and placement. It may
  contain a base-local lane identifier, but that identifier is deliberately
  absent from Plan identity and serialization.

Optional topology groups can truthfully expose NUMA domains, cache domains, and
performance classes. An empty topology list means that topology is unknown or
not contractual. It cannot satisfy a hard topology requirement.

## Hosted and bare-metal realization

`HostedOs` and `BareMetal` bases realize the same lane entitlement. A hosted
base can use admitted OS workers. A bare-metal base can enumerate,
start, wake, run, park, and signal execution lanes through the generic base
boundary. Neither base may invent capacity, placement, retries, or
authority, and neither becomes the Conduit scheduler.

Implementation identity, architecture-specific artifact identity, compute-pool
identity, selected reservation, and runtime backend or lane assignment remain
distinct. The same implementation may therefore advertise different artifacts
for different architectures while retaining an equal checked face.

## Proof limits

The deterministic planner tests prove bounded minimum-first allocation, exact
service and topology selection, hosted/bare-metal contract parity, Plan identity
sealing, and the exclusion of physical lane identifiers from serialized Plans.
They do not claim operating-system scheduling quality, bare-metal interrupt
behavior, firmware execution, or physical/HIL clue.

## Non-AI generality check

The realization machinery above contains no AI-specific core concept. The same
checked-face, offer, observation, hard-requirement, policy, reservation, and Plan
identities encode the following two examples without changing the planner.

### Video transcoding: CPU or GPU

An authored operation requests a checked `media/transcode-video` face with exact
bounded input/output ports and semantic codec/output limits. Two hosts can offer
that equal face with different nominal revisions and exact realizations:

| General R2 fact | CPU realization | GPU realization |
| --- | --- | --- |
| `ImplementationOffer` | portable software encoder | accelerated encoder |
| `artifact_id` | architecture-specific CPU binary | architecture-specific GPU binary |
| `ResourceRequirement` | bounded shared/reserved compute lanes plus memory | bounded compute lanes, accelerator execution, and accelerator memory |
| stable characteristics | codec/profile ceiling, local handling, measured throughput class | codec/profile ceiling, local handling, different measured throughput class |
| current observations | unreserved CPU lanes and memory | unreserved accelerator slots and memory |

A hard codec/profile or memory requirement removes an incapable realization
before ranking. With both admitted, explicit policy can prefer locality, fewer
resource units, a measured throughput class, or a stronger compute-service
guarantee. The Plan seals the selected host, implementation, artifact, resource
bindings, semantic limits, and characteristics. It never changes the authored
face into `CUDA`, `VA-API`, or a device name, and it never seals a transient GPU
queue/core identifier.

### Storage write: local disk or network storage

An authored operation requests a checked `storage/write-object` face with a
finite byte bound and explicit terminal behavior. A local-filesystem base
and a network-object base can advertise that equal face:

| General R2 fact | Local realization | Network realization |
| --- | --- | --- |
| `ImplementationOffer` | local filesystem writer | remote object writer |
| finite resources | storage bytes, execution slot | storage bytes, execution slot, network egress |
| authority requirements | exact local protected-resource role | exact remote subject/credential role and egress authority |
| stable characteristics | local handling, replace/create support | remote handling, metering/durability class where proven |
| current observations | local capacity/health | remote slot/egress readiness |

A hard no-egress rule or authority allowlist rejects the network realization;
it cannot win through favorable durability or capacity policy. When both are
admissible, explicit policy can select one deterministically. The Plan seals the
exact resource and authority bindings, but credential bytes, endpoint secrets,
open file descriptors, sockets, and base request IDs remain outside it.

In both examples, changed observations can produce a newly admitted replacement
Plan while the old Plan remains immutable. Neither example introduces a media
planner, storage planner, opaque host score, opportunistic runtime substitution,
or a second execution kernel.
