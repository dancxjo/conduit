# Portable catalog and hosted std offer boundary

`conduit-semantic-catalog` is the remaining host-neutral semantic catalog. It owns
portable kind fronts, configuration rules, finite semantic limits, canonical
catalog installation, and deterministic calculations. It is not a host and it
does not advertise that any implementation is currently available.

The hosted reference host owns its exact realization identities and
`CapabilityOffer` construction in `targets/std/offers` (`conduit-std-offers`).
Its `supported_nucleus_offers()` inventory must preserve every selected
portable front exactly while adding the std execution profile, implementation,
artifact, Host Call, resource, and authority facts required to execute it.
Runtime advertisement remains planner truth; catalog membership does not imply
availability.

The boundary is therefore:

```text
portable kind contract and value meaning
    conduit-semantic-catalog and focused semantic crates

hosted std realization offer
    targets/std/offers

installed hosted operation and host adapter
    targets/std
```

Education, vision, structured robotics, job/reminder workflows, recurrence and
calendar calculation, and direct Patchbay presentation follow the same rule.
Provider-specific physical motion authority is not a portable default offer.
Browser and ConduitOS construct their own realization offers from the portable
contracts and do not inherit hosted std implementation truth.

The historical ten-operation S5 table is no longer the executable inventory.
The checked authorities are `supported_nucleus_contracts()` for portable
contract selection and `conduit_std_offers::supported_nucleus_offers()` for the
hosted realization set. Tests require exact kind, revision, port, and bound
agreement between the two and reject hosted realization identities in the
neutral catalog sources.

No dynamic registry or runtime plugin mechanism is involved. Both composition
and advertisement remain deterministic and finite.

## Working on the catalog

Keep portable contracts and hosted offers in their respective owners, and run
the relevant catalog/offer conformance checks through the repository development
entrance. See the [contribution guide](../../CONTRIBUTING.md). Inventory counts
are derived from code rather than maintained as a second table in this note.
