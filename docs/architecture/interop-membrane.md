# External-system sovereignty and the interop membrane

Status: generic architecture and deterministic core proof for #3099.
Entrance: `cargo xtask check interop-membrane`.

Conduit may integrate with external ecosystems without absorbing their
identities or authority. `InteropMembrane` records a finite set of exact,
directional mappings. Each mapping keeps separate the external resource,
Conduit Kind/revision, adapter, Base, mapping identity, authority grant, finite
payload/queue bounds, delivery contract, and lifecycle behavior.

Import and export are independent mappings. External discovery is only an
observation submitted to `consider_import`; it creates no Host, Part,
capability, trust, membership, or authority. Unconfigured siblings return
`Unconfigured`. Payload overflow remains distinct. A mapping whose declared
delivery or lifecycle truth differs from the external contract refuses before
registration rather than fabricating stronger semantics.

Every outward manifestation receives an exact origin made from adapter,
export-mapping, and manifestation identities. When discovery observes that
same exact origin/resource pair, the default result is
`ReflectedManifestation`, not a fresh imported capability. Friendly names play
no role. Intentional re-import requires a separately registered import mapping
that names the exact export mapping and carries its own fresh authority grant.

The core fixture imports configured resource A, manifests an ordinary semantic
result as B, observes B again, and proves that B is reflection-fenced while
unconfigured sibling C stays unavailable. It then registers an explicit fresh
mapping for B and proves the new mapping/authority identities remain distinct.
The contract is ecosystem-neutral so ROS, browser, service, database, device,
and future adapters consume the same boundary rather than inventing private
loop or authority systems.

This proof establishes deterministic identity, direction, bounds, and loop
decisions. A concrete adapter must additionally prove its actual external
provider enforcement and preserve the external ecosystem's own security model.
