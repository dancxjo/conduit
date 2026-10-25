# Compiler cache implementation status

The cache has an immutable store, an owned engine controller, and an initial real
Conduit Cargo integration. It is not installed as a global Cargo wrapper or used
to authorize behavioral/release proof reuse.

`tools/ci/compiler-cache/store.mjs` is an immutable, content-addressed object
store. Its key requires source, toolchain, features, test and environment
custody. It supports explicit eviction, finite quotas and process-identity
leases; eviction preserves leased objects. These identity strings are caller
inputs, so completeness must be established by the eventual build integration.

`runtime.mjs` acquires a pinned Linux x86_64 sccache engine with verified archive
and executable digests. Each session owns one foreground server, an exclusive
cache lease, a private socket and a new task directory. Session cleanup reaps its
server and process groups. A crash preserves the lease for audited recovery.
The engine supplies its disk quota; a monitor checks disk reserve and task/cache
size. This is monitored admission and refusal, not a filesystem hard quota.

The initial compilation API accepts a caller-reviewed immutable input closure
and fixed nonincremental rlib arguments. It checks file/compiler hashes before
and after compilation. It does not derive Cargo, procedural-macro, toolchain
sysroot or filesystem closure completeness. A reviewed string is not a sandbox.

`probe.mjs` demonstrates one actual cold compiler miss and one warm hit across
two separately owned server sessions, with identical rlib bytes. The small probe
does not demonstrate a material speedup or an integrated Conduit build benefit.
The supported proof entrance below supplies explicitly acquired paths to the
engine lifecycle and actual Cargo tests.

Broader Cargo integration requires immutable source/vendor/toolchain/sysroot
snapshots, explicit features/profile/environment policy, private generated
outputs, and compiler execution inside that boundary. A host-side daemon may
execute the compiler outside a Cargo sandbox, so an outer sandbox alone is
insufficient. Procedural-macro consumers must bypass caching until their full
input boundary is established. Behavioral tests and product proofs continue to
execute independently of compiler object hits.

The first supported Cargo slice builds the actual dependency-free
`conduitos-compositor-core` package with its original manifest. It snapshots exact
tracked Git bytes, the host compiler/Cargo/sysroot, selected dynamic runtime and
pinned engine; a generated standalone lockfile is included in the snapshot
identity. Cargo runs offline/frozen inside bwrap. The compiler server and rustc
children run inside that same read-only snapshot boundary. Only a private task
directory and managed compiler objects are writable; host-only file reads refuse.
The namespace includes snapshot, runner/mount/environment policy and bwrap binary
identities. New dependencies/build scripts/procedural macros refuse instead of
being admitted under this package's contract.

Use scratch paths outside the source checkout:

```sh
cargo xtask ci pipeline compiler-cache-engine /scratch/new-engine
cargo xtask ci pipeline compiler-cache-prepare /scratch/new-engine/sccache /absolute/rust-toolchain /scratch/new-snapshot /scratch/new-snapshot.json
cargo xtask ci pipeline compiler-cache-build /scratch/new-snapshot.json /scratch/compiler-cache /scratch/cold-task
cargo xtask ci pipeline compiler-cache-build /scratch/new-snapshot.json /scratch/compiler-cache /scratch/warm-task
cargo xtask ci pipeline compiler-cache-clear /scratch/compiler-cache EXACT_NAMESPACE /scratch/new-eviction.json
cargo xtask ci pipeline compiler-cache-proof /scratch/new-snapshot.json /scratch/new-engine/sccache /absolute/rust-toolchain/bin/rustc
```

Prepare requires a clean exact-source checkout and rechecks copied bytes against
Git. Build verifies the retained source/closure and keeps Cargo targets private.
The initial engine cache is 16 MiB, with explicit task/reserve limits. Clear
requires the exact namespace, an exclusive lease and audited managed engine
objects. It refuses active engines/visible object descriptors, unknown files,
symlinks and interrupted retirement. Task outputs and proof receipts are outside
the cleared store. It does not break stale leases or guess how to resume an
interrupted transaction.

Actual tests demonstrate miss → hit → clear → miss with identical rlib/rmeta
bytes. The measured two-build example was approximately 1.55 s cold and 1.24 s
warm including closure hashing; snapshot acquisition is additional. This proves
the selected Cargo path, not whole-workspace acceleration. General dependency
graphs, procedural macros, linking and automatic CI cache wiring remain work.
