# Browser Host identity and Body membership

The durable browser Host profile treats one browser installation/profile as one Host across ordinary page reloads:

```text
durable HostId       stored by the Host, independent of applications
BootId               fresh for every initialized runtime incarnation
application state    separately keyed and independently forgettable
Body membership      Body-owned admitted Part and signed membership events
```

The Host keeps exactly one bounded `conduit.browser/host-identity@1` record. It contains the stable `HostId` and continuity signing seed. A missing record is initialized once from the browser cryptographic RNG. A malformed record, quota failure, or database version failure refuses Host startup; it is not silently replaced. `resetBrowserHostIdentity()` is the separate, explicit operation that rotates this identity. Clearing Book, Crèche, or Patchbay state never calls it.

Each runtime initialization creates a fresh `BootId`. Restored application state cannot make an earlier Boot current. When the durable Host is already an admitted Part, the Body records the prior Boot as `HostDetached` and the fresh Boot as `HostAttached`, with exact monotonically revised membership and biography evidence. A different Host identity refuses reconciliation without changing the restored membership.

`conduit.browser/body-membership-client@1` is the reusable browser-side admission client. It exposes the exact current Host advertisement and signs only bounded Body-issued admission, return, or spawn challenges for its own Host and Boot. Knowing a `BodyId` is insufficient. The Body admission authority remains responsible for validating the invitation/proof and applying ordinary `admit`, `observe_present`, `observe_offline`, or `revoke` transitions. Crèche and Patchbay consume this client; neither owns a parallel membership algebra.

Leaving, removal, and forgetting are distinct:

- **Leave** records the current Boot offline while its Part remains admitted.
- **Remove this browser** revokes that Part; it does not destroy the Body.
- **Finish/forget an application** deletes only that bounded application's local association; it does not mutate Body membership or reset Host identity.

An external-reader/controller Patchbay does not invoke the membership client. Its browser Host remains outside the viewed Body and uses separately admitted Observatory/control boundaries.

The explicitly ephemeral browser profile creates a fresh Host and Boot and makes no continuity claim. It is not a fallback for corruption of the durable profile.
