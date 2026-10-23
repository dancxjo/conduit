# Final prepared lifecycle schema: hosted evidence only

Frozen tested source: `831d3b0a66cbb4b03ae55629a6a0de505e9ba3a8`.
This documentary descendant retains 32 actual focused hosted test results:
28 session-owner/lifecycle/serializer tests, followed once by four display-grant
tests using the identical retained executable. Zero tests failed or were ignored.
The filters excluded 1323 and 1347 tests respectively; this is not a full suite.

The six explicit serializer controls cover maximum numeric fields, escaped
identifiers, every closed phase/kind with explicit nulls, capacity transitions,
overflow replacement, exact borrowed 32-byte guest digest serialization and
allocation-free prepared emission. The allocation control prepares borrowed
records and fixed buffers before measurement and parses after measurement. It
observes zero alloc/alloc_zeroed/realloc calls for maximum normal serialization
and capacity/overflow fallback, plus zero net rounded retained bytes. The helper
has no total requested-byte counter: zero allocation calls establishes that
emission requested no heap allocation. This does not measure whole product
allocation or include preparation/parsing.

`current-831d3b/run.json` binds exact argv, environment, source clean after-state,
first output and actual test ELF. The retained binary is 156,357,392 bytes with
SHA-256 `59ef29325fbb78279fd6e02da8ee1d6efe9923f234e19d736b34eb00728507e2`.
It remains outside Git at its recorded absolute path. The display-grant command
in `identity-run.json` ran directly from that same binary without rebuilding.
Compiler version was queried after tests during packet sealing, not captured
independently before execution. No new builds were performed for publication.

This proof does **not** compile the native raw-clock path, DisplayOwner's
one-time embedded-image digest computation or native borrowed identity
constructor. It proves hosted serialization of supplied digest bytes, not that
the running product supplied its actual guest digest. Native compilation,
collector integration, actual product/CPL3/QEMU execution, full owning checks,
matched timing and complete #6185 acceptance remain pending. No speedup,
physical, stable-release or complete product proof is claimed.

`historical-sources.json` links the immutable earlier 42dc/f983 evidence packet
and labels prior ce1877 results by their exact source. Prior ce1877 raw output
and metadata are retained here; its 26 tests are not relabelled as831 results.
`prior-42dc-source-review.json` is explicitly an earlier source review, not the
final831 review. The separately named current Root review concerns831 source.

The first982 attempt's linker SIGBUS, exit101, raw output and metadata remain
under `negative-982af9`. No tests executed on that source. The later shared
cache disappearance remains an observation with unproven causal connection;
this packet does not claim an unchanged retry fixed it. A pre-Cargo
cross-filesystem archival refusal is also preserved. Later successes used
changed sources and the deliberately isolated cache under explicit ownership.

All `.gz` files preserve exact original bytes losslessly. `hash-manifest.json`
binds every packet file except itself, including compressed and uncompressed
hashes. Binaries are intentionally retained outside Git. Current source,
prior sources, negative evidence and scope limits remain distinct.
