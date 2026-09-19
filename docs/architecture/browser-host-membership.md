# Browser host identity and body membership

The durable browser host profile treats one browser installation/profile as one host across ordinary page reloads:

```text
durable HostId       stored by the host, independent of applications
BootId               fresh for every initialized runtime incarnation
application state    separately keyed and independently forgettable
body membership      body-owned admitted part and signed membership events
```

The host keeps exactly one bounded `conduit.browser/host-identity@1` record. It contains the stable `HostId` and continuity signing seed. A missing record is initialized once from the browser cryptographic RNG. A malformed record, quota failure, or database version failure refuses host startup; it is not silently replaced. `resetBrowserHostIdentity()` is the separate, explicit operation that rotates this identity. Clearing Book, Crèche, or Patchbay state never calls it.

Each runtime initialization creates a fresh `BootId`. Restored application state cannot make an earlier boot current. When the durable host is already an admitted part, the body records the prior boot as `HostDetached` and the fresh boot as `HostAttached`, with exact monotonically revised membership and biography evidence. A different host identity refuses reconciliation without changing the restored membership.

`conduit.browser/body-membership-client@1` is the reusable browser-side admission client. It exposes the exact current host advertisement and signs only bounded body-issued admission, return, or spawn challenges for its own host and boot. Knowing a `BodyId` is insufficient. The body admission authority remains responsible for validating the invitation/proof and applying ordinary `admit`, `observe_present`, `observe_offline`, or `revoke` transitions. Crèche and Patchbay consume this client; neither owns a parallel membership algebra.

Leaving, removal, and forgetting are distinct:

- **Leave** records the current boot offline while its part remains admitted.
- **Remove this browser** revokes that part; it does not destroy the body.
- **Finish/forget an application** deletes only that bounded application's local association; it does not mutate body membership or reset host identity.

An external-reader/controller Patchbay does not invoke the membership client. Its browser host remains outside the viewed body and uses separately admitted Observatory/control boundaries.

The explicitly ephemeral browser profile creates a fresh host and boot and makes no continuity claim. It is not a fallback for corruption of the durable profile.
