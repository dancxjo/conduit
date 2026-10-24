# Immutable snapshot records

Shared no-std/alloc extraction of the existing browser snapshot record codec,
for eventual symmetric browser and Linux recorder Hosts. The browser keeps its
public re-exports and wire effect shape. The optional `host-call` feature owns
the shared typed request correlation and strict reference/JSON validation;
`serde` adds the existing effect serialization. The wire format,
key derivation and authority/reference checks are unchanged. The codec owns no
filesystem, IndexedDB, scheduler, transport or durable catalogue seed.

Preparation checks the exact selected residence, authority, Host/Boot, immutable
reference version and finite extent. Publication and restoration still require
the retained matching authority; a record or portable reference cannot grant it.
The record envelope retains the existing finite 4-KiB content ceiling. Actual
storage execution and acknowledgement remain independently admitted effects.

`cargo xtask check resource-snapshot` runs the shared record tests, existing
browser operation tests and strict codec Clippy. These are CPU contract checks;
this boundary alone establishes no admitted Linux store or completed Pi recorder.
