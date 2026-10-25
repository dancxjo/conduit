# Compiler cache implementation status

The cache has two development layers. Neither is installed as a global Cargo
wrapper or used to authorize release proof reuse.

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
Run the meaningful engine lifecycle tests with explicitly acquired paths:

```sh
CONDUIT_CACHE_PROOF_ENGINE=/absolute/pinned/sccache \
CONDUIT_CACHE_PROOF_RUSTC=/absolute/toolchain/bin/rustc \
node --test proof/ci/compiler-cache.spec.mjs proof/ci/compiler-cache-runtime.spec.mjs
```

Actual Cargo integration requires immutable source/vendor/toolchain/sysroot
snapshots, explicit features/profile/environment policy, private generated
outputs, and compiler execution inside that boundary. A host-side daemon may
execute the compiler outside a Cargo sandbox, so an outer sandbox alone is
insufficient. Procedural-macro consumers must bypass caching until their full
input boundary is established. Behavioral tests and product proofs continue to
execute independently of compiler object hits.
