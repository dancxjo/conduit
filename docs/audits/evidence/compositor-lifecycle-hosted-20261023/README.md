# Compositor lifecycle observations: hosted evidence only

Measured source: `42dc6b201a6fe900725882eb9eef922a31abc3f5`.
This documentary descendant preserves 26 actual focused hosted test results:
22 session-owner/lifecycle tests and four display-grant tests. None were ignored.
The first run filtered 1323 tests; the second filtered 1341. This is not the
full owning package/workspace suite, native/CPL3/QEMU proof, product timing,
performance evidence, stable acceptance, or completion of #6185.

The first focused command compiled the clean frozen source once using a new
isolated `/tmp/conduit-6185-lifecycle-target-42dc6b`. Its exact argv/environment,
capacity checks, zero exit, clean after-state and retained actual test ELF
identity appear in `positive-42dc6b/run.json`. The four display identity/scope
checks then ran once directly from that retained identical test executable;
`identity-run.json` records the command/hash and zero exit. No rebuild occurred
between those two filters. Tests cover absent/reverse clocks, explicit count
overflow, retained activation identity and terminal failure behavior, plus
exact Host/Boot/offer/Base/format matching and replacement refusals.

The actual 156,221,992-byte test ELF remains preserved outside Git at the path
in the manifest. Its SHA-256 is
`918165b3ba24ee3f312e4b9e0a2fa67fa64cb00f08b9812ad9e4a2004710362f`.
Root's independent source review is retained. Compiler version was queried
after the tests while sealing this packet; that observation is explicitly
labelled and is not a separately captured pre-execution compiler receipt.

Earlier source `982af9518563b4485f615835466c459197acc221` failed its first
focused hosted attempt at linking with SIGBUS (exit 101). No tests executed.
Its raw output and source/command metadata are retained under `negative-982af9`.
The preceding cross-filesystem archival preflight refusal happened before any
Cargo execution; its note is retained too. The old shared cache subsequently
disappeared externally; the SIGBUS cause remains unproven. The positive run
used changed source and a fresh isolated cache. It does not erase the failure,
prove an unchanged retry succeeded, or establish the cause of the old failure.

All `.gz` files losslessly preserve original bytes with deterministic gzip
metadata. The manifest binds compressed and uncompressed hashes, metadata and
results. No binaries are committed. First-failure evidence remains negative;
collector integration, full owning checks and real native product proof remain
separate pending work. No timing values were produced by these hosted tests:
the optional native raw clock correctly reports unavailable on hosted runs.
