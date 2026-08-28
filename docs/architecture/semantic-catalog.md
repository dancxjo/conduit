# Portable catalog and hosted std offer boundary

`conduit-semantic-catalog` is the remaining host-neutral semantic catalog. It owns
portable Kind faces, configuration rules, finite semantic limits, canonical
catalog installation, and deterministic calculations. It is not a Host and it
does not advertise that any implementation is currently available.

The hosted reference Host owns its exact realization identities and
`CapabilityOffer` construction in `targets/std/offers` (`conduit-std-offers`).
Its `supported_nucleus_offers()` inventory must preserve every selected
portable face exactly while adding the std execution profile, implementation,
artifact, Host-operation, resource, and authority facts required to execute it.
Runtime advertisement remains planner truth; catalog membership does not imply
availability.

The boundary is therefore:

```text
portable Kind contract and value meaning
    conduit-semantic-catalog and focused semantic crates

hosted std realization offer
    targets/std/offers

installed hosted operation and Host adapter
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
hosted realization set. Tests require exact Kind, revision, port, and bound
agreement between the two and reject hosted realization identities in the
neutral catalog sources.

No dynamic registry or runtime plugin mechanism is involved. Both composition
and advertisement remain deterministic and finite.

## Audit checks

```text
cargo test -p conduit-semantic-catalog
cargo test -p conduit-std-offers
cargo check -p conduit-semantic-catalog --no-default-features --target thumbv6m-none-eabi
```
