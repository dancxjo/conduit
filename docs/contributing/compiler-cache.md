# Compiler cache implementation status

The supported cache builds the actual dependency-free
`conduitos-compositor-core` Cargo library on Linux x86_64. It does not install a
global Cargo wrapper or authorize behavioral/release proof reuse. The hosted
Linux extension below is an incomplete internal integration checkpoint; its full
cold/warm release and Hello acceptance must complete before pipeline adoption.

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


## Hosted Linux extension: incomplete integration checkpoint

`hosted-snapshot.mjs` prepares the actual workspace for
`cargo build --locked --release -p conduit --bin conduit`.
`runHostedCargoCache` uses the existing exclusive lease, owned in-boundary
sccache server, bounded object storage and audited clear contract. Each build
gets a new private Cargo target. `hosted-proof.mjs` executes the existing canonical
Hello protocol check separately against the retained release executable and
verifies that executable is unchanged. These are internal composition APIs;
the supported pipeline CLI still selects the compositor profile until the
integration owner wires and accepts the hosted profile.

Preparation runs Cargo metadata offline. Acquire the locked public dependencies
before freezing; missing cached archives or Git source refuse. Registry source
bytes come from freshly extracted `.crate` archives authenticated against every
registry entry in the exact Cargo.lock, including optional entries omitted by
metadata. Mutable registry source directories are not used as compiler inputs.
Archive symlinks, hardlinks, traversal and oversized inputs refuse.

The pinned Tongues Git workspace preserves its shared fixture layout. Its raw
commit, trees and every retained regular blob are authenticated. The six public
links to external private data are recorded as absent and are never followed or
mounted. Pinned Cargo-normalized manifests for the two selected Git packages
have explicit generated-input custody; updating that Git revision requires
regenerating and reviewing those manifests. Other public Git bytes remain exact.

The current native closure is concrete Debian x86_64: GCC 15, binutils 2.46,
selected libc/kernel headers and explicit ELF dependencies, alongside the exact
Rust compiler/Cargo/sysroot and Rust linker. Missing platform files refuse;
there is no fallback mount of host `/usr`, `/home` or `/etc`. Other hosted runner
images need their own reviewed native closure before CI adoption. Preparation
requires 19 GiB free and caps the retained snapshot at 6 GiB / 150,000 files.

Before compilation, Cargo resolves the frozen inputs offline inside bwrap and
checks that their procedural-macro closure is covered by the captured bypass
policy. Every transitive macro consumer bypasses sccache, including macros
reexported behind ordinary rlibs. This intentionally includes conservative
workspace feature/development edges and can exclude expensive Conduit crates
from caching. Response files and unclassified dynamic macro inputs refuse.
Build scripts execute normally; their generated outputs are retained and hashed.
No macro consumer is admitted by a string claiming its inputs are complete.

The initial hosted limits are 4 GiB compiler objects, 12 GiB private task output
and 10 GiB free reserve. Both task and cache filesystems are checked. Limits are
monitored admission/refusal bounds, not filesystem hard quotas. Receipts retain
actual sccache counters, output hashes, Cargo wall time and GNU time resource
usage; maximum resident size is the largest measured child, not an aggregate
concurrent-memory peak. Hello has its own execution receipt and timing.

The namespace binds the entire retained snapshot and runner/mount/environment
policy. This proves reuse of one exact retained snapshot; it does not establish
cross-revision or separately prepared snapshot reuse. Procedural-macro bypass,
namespace conservatism and preparation costs must be included when deciding
whether the profile saves useful CI time. Completed hosted speed measurements
and automatic CI wiring are still acceptance work at this checkpoint. The
[engine Rust caveats](https://github.com/mozilla/sccache/blob/v0.19.0/docs/Rust.md)
also exclude system-linker crate invocations from cache reuse.
