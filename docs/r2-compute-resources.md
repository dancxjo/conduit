# R2 compute resources

Compute is an ordinary finite resource in the existing host-advertisement,
planning, reservation, and kernel-preparation path. It is not a second
allocator, scheduler, planner, or execution API.

## Contract boundaries

- A host advertises architecture-neutral compute pools. Physical packages,
  cores, hardware threads, and provider scheduler identifiers remain provider
  facts rather than authored form facts.
- A capability requirement states minimum, preferred, and maximum lanes, a
  minimum service guarantee (`Shared`, `Reserved`, or `Exclusive`), and optional
  topology constraints. The planner admits every minimum before assigning spare
  capacity toward preferences.
- A sealed Plan records the selected lane count, pool, service guarantee,
  architecture-provider identity and kind, and any selected stable topology
  group. Current utilization and concrete lane assignment are separate runtime
  facts.
- A transient lane assignment belongs to an active play and placement. It may
  contain a provider-local lane identifier, but that identifier is deliberately
  absent from Plan identity and serialization.

Optional topology groups can truthfully expose NUMA domains, cache domains, and
performance classes. An empty topology list means that topology is unknown or
not contractual. It cannot satisfy a hard topology requirement.

## Hosted and bare-metal realization

`HostedOs` and `BareMetal` providers realize the same lane entitlement. A hosted
provider can use admitted OS workers. A bare-metal provider can enumerate,
start, wake, run, park, and signal execution lanes through the generic provider
boundary. Neither provider may invent capacity, placement, retries, or
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
behavior, firmware execution, or physical/HIL evidence.
