# Compiler cache implementation status

The supported cache builds the actual dependency-free
`conduitos-compositor-core` Cargo library on Linux x86_64. It does not install a
global Cargo wrapper or authorize behavioral/release proof reuse. Broader Cargo
graphs, procedural macros, linking and automatic CI cache wiring remain work.

`engine.mjs` acquires pinned sccache 0.19.0 into a new task-local directory and
verifies both archive and executable SHA-256 digests. `cargo-snapshot.mjs`
snapshots exact tracked package bytes, compiler/Cargo/host sysroot, selected
ELF runtime and the pinned engine. A generated standalone lockfile is included
in snapshot custody. New dependencies, build scripts and procedural macros
refuse under this package's contract.

Cargo runs offline/frozen inside bwrap. The owned sccache server and rustc
children run inside the same read-only snapshot boundary; compilation cannot
silently move to a host-side daemon. Only a private task directory and managed
compiler objects are writable. Cargo targets are never shared. The namespace
includes snapshot, runner/mount/environment policy and bwrap binary identities.

Use new scratch paths outside the source checkout:

```sh
cargo xtask ci pipeline compiler-cache-engine /scratch/new-engine
cargo xtask ci pipeline compiler-cache-prepare /scratch/new-engine/sccache /absolute/rust-toolchain /scratch/new-snapshot /scratch/new-snapshot.json
cargo xtask ci pipeline compiler-cache-build /scratch/new-snapshot.json /scratch/compiler-cache /scratch/cold-task
cargo xtask ci pipeline compiler-cache-build /scratch/new-snapshot.json /scratch/compiler-cache /scratch/warm-task
cargo xtask ci pipeline compiler-cache-clear /scratch/compiler-cache EXACT_NAMESPACE /scratch/new-eviction.json
cargo xtask ci pipeline compiler-cache-proof /scratch/new-snapshot.json
```

Prepare requires a clean exact-source checkout and checks copied bytes against
Git. Build verifies retained source/closure identities and records the cache
namespace and output hashes in each task's receipt. Pass that exact namespace
to clear. The proof command exercises actual Cargo miss → hit → clear → miss,
identical rlib/rmeta bytes, host-only read refusal and failure cases; acquisition
refusal tests run without network downloads.

The initial engine cache is 16 MiB, with explicit task/reserve limits. These are
monitored admission and refusal bounds, not filesystem hard quotas. Sessions
hold an exclusive lease; crashes retain it for audited recovery. Clear audits
managed engine objects and refuses active engines/visible object descriptors,
unknown files, symlinks and interrupted retirement. Task outputs and proof
receipts remain outside the cleared store. Clear does not break stale leases or
guess how to resume an interrupted transaction.

A measured two-build example was approximately 1.55 s cold and 1.24 s warm,
including closure hashing; snapshot acquisition is additional. This establishes
the selected Cargo path, not whole-workspace acceleration. Behavioral tests and
product proofs still execute independently of compiler object hits. The
[sccache Rust caveats](https://github.com/mozilla/sccache/blob/v0.19.0/docs/Rust.md)
and [single-server local cache contract](https://github.com/mozilla/sccache/blob/v0.19.0/docs/Local.md)
remain relevant when extending this slice.
