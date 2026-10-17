# Printer acceptance tiers and operator prerequisites

The supported deterministic proof is:

```sh
cargo xtask check print-pool
```

Run it from a checkout of the reviewed revision and retain that checkout's
commit identity with the command result. The suite checks typed document/support
admission, hard printer attributes, generic shared-pool cost and queue selection,
provider availability, bounded waiting, requester fairness and the finite
append-only request fence. The fence retains terminal or uncertain job identities
and refuses duplicate dispatch; it does not provide durable reconciliation or
exactly-once physical delivery. The suite performs no
print I/O. The fixture provider is explicitly a protocol model; it does not
install or impersonate an effectful `print/submit` Host Back.

The ordinary Source example is `plots/print-contract/main.conduit`. Its public
checker entry is documented in that directory. A clean checker result proves
Source validity, not printer availability. The default Host currently has no
print provider, and ordinary placement refuses the missing capability. The
product integration test `products/conduit/tests/print_contract.rs` checks both
facts independently of the fixture scheduler proof.

## Physical tier remains unavailable

There is no installed CUPS/IPP print adapter in this delivery. Having `lp` or
`lpstat` installed does not create an admitted Back, document authority or a
provider acknowledgement. Do not bypass the Host Call boundary with a shell
submission and label it a Plan/Play proof.

A read-only local preflight on 2026-10-16 found both commands installed, but
`lpstat -p -d` returned exit 1 with “No destinations added” and “no system default
destination”. No document was submitted. This is a configured-printer blocker
in addition to the absent adapter, not proof that every possible printer or
remote endpoint is unavailable.

A future physical run requires an installed supported Host Back, an explicitly
selected non-sensitive document and device, exact current document resource and
authority admission, and an operator's explicit opt-in before dispatch. Retain
Source, checked/expanded Plot, Plan/Play, document identity/version and provider
job/ACK receipts. Dispatched, acknowledged and physically completed are separate
facts; lost acknowledgement or provider loss must remain uncertain. Never
retry an irreversible uncertain print automatically or claim exactly-once
physical output from an in-memory protocol fence.

Issue #5559 and the physical gate in parent #5229 remain open. This document
records prerequisites and proof boundaries; it does not supply the missing
adapter, a successful device run or permission to print.
