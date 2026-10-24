# Immutable snapshot records

Extraction of the existing browser immutable record codec into a shared no-std/
alloc mechanism. Record bytes, key derivation, reference version/extent and
independent authority/residence checks are unchanged. Browser public paths are
re-exports. The existing record fixture is shared rather than copied, with an
additional owner-metadata bound regression test.

`cargo xtask check resource-snapshot` checks the generic codec and browser public
path plus strict codec Clippy. This creates no storage executor, native provider,
Host offer, catalogue seed, scheduler or Body. Linux file effects and shared typed
operation integration remain separate delivery. Refs #6350.
