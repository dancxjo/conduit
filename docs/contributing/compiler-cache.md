# Compiler cache implementation status

The supported cache builds the actual dependency-free
`conduitos-compositor-core` Cargo library on Linux x86_64. It does not install a
global Cargo wrapper or authorize behavioral/release proof reuse. The hosted
Linux extension below is an incomplete internal integration checkpoint; its full
cold/warm release and Hello experiment has completed locally; portable CI and
cross-revision acceptance remain required before pipeline adoption.

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

Preparation runs Cargo metadata offline. Frozen metadata canonicalizes Cargo
package IDs and source/target paths to their actual sandbox identities; raw
acquisition paths and bytes remain in a private sibling `.acquisition.json`
receipt outside the hashed snapshot. Unsupported external acquisition paths
refuse. Acquire the locked public dependencies
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

The native closure selects the installed x86_64 GNU GCC 12–15 private
compiler layout, concrete linker/startup/header files and their resolved ELF
dependencies, alongside exact Rust compiler/Cargo/sysroot/linker bytes. This
handles GCC 13 under Ubuntu `/usr/lib/gcc` and GCC 15 under Debian
`/usr/libexec/gcc` without hardcoding a binutils soname. Missing platform files
and unknown loader output refuse; there is no fallback mount of host `/usr`,
`/home` or `/etc`. Two independently prepared local GCC 15 snapshots at distinct
paths were byte-identical (56,270 files / 2,226,552,902 bytes); the copied C
compiler, assembler, linker and runtime passed a contained native execution and
refused an existing host-only include. Actual Ubuntu execution still requires
CI acceptance; layout fixtures alone do not establish it. Preparation requires
19 GiB free and caps the retained snapshot at 6 GiB / 150,000 files.

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
actual sccache counters, output hashes and Cargo wall time. The retained GNU
time output measures the external bwrap launcher on this host and must not be
reported as compiler CPU or memory. The measurement harness separately samples
owned sandbox processes. CPU totals are sampled lower bounds; concurrent
summed RSS can count shared pages more than once and is not exact peak
physical RAM. Hello has its own execution receipt and timing.

The namespace binds the entire retained snapshot and runner/mount/environment
policy. This proves reuse of one exact retained snapshot; it does not establish
cross-revision or separately prepared snapshot reuse. Procedural-macro bypass,
namespace conservatism and preparation costs must be included when deciding
whether the profile saves useful CI time. One local uninterrupted fresh-target comparison of the exact retained source
`6e55fb036498db24d1e1503ef7bf828e3978b499` took 683.62 s warm and 704.60 s
after explicit cache clear: 88 Rust hits versus 88 misses/writes, about 21 s /
3% observed wall-time difference. All 514 rlib/rmeta/executable hashes matched.
The 61,653,536-byte Conduit binary independently passed unchanged canonical
Hello in 0.403 s warm and 0.386 s cold. Preparation is additional (a separately
prepared newer source took 22.05 s using already acquired dependency archives;
network acquisition was not timed). These are one local pair, not a statistical
speed claim or elimination of expensive runtime tests. Most Conduit compilation
still bypasses the cache. Automatic CI wiring and useful cross-revision
acceptance remain incomplete. The
[engine Rust caveats](https://github.com/mozilla/sccache/blob/v0.19.0/docs/Rust.md)
also exclude system-linker crate invocations from cache reuse.

The internal build and Hello APIs accept a synchronous `observeCommand` callback.
Each actual spawned process reports its program, complete arguments, host cwd,
minimal effective host environment, duration and success/failure. In-sandbox
commands/environment remain explicit in the bwrap arguments; the observer does
not relabel the external supervisor as a host Cargo invocation. Custody callback
failures refuse acceptance even if compilation itself succeeded.

## Audited dependency sharing: incomplete acceptance

`prepareHostedSnapshot` accepts explicit `cacheMode: 'audited-dependencies'`
and an optional absolute `acquisitionFile` outside the source, snapshot,
toolchain and Cargo home. Its default remains `exact-snapshot`.
`hostedCachePolicy(snapshot, { maxCacheBytes, cacheMode })` validates the full
frozen snapshot and admission policy before returning opaque `{ policy,
namespace }` for restore. The build uses that same derivation; changing the
frozen mode refuses. Clear understands both exact schema 2 and shared schema 3.

The shared namespace binds all vendor, compiler/sysroot, engine, native/runtime,
source `.cargo` configuration, admission and sandbox/wrapper policy bytes. Each
task still binds its complete source commit/tree and snapshot. The same stable
namespace enters Cargo's compiler-cache environment. Unrelated source changes
can therefore retain dependency compatibility without replacing source custody.

The finite profile pins `syn` 2.0.119/3.0.6, `clap_builder` 4.6.7 and `winnow`
0.7.15 by authenticated archive and complete source-inventory hashes. These
library sources have no own build script, procedural macro target or assembly
input mechanism. Their builtin documentation includes remain inside immutable
vendor bytes; Cargo env macros and extern artifacts use pinned sccache keys.
All transitive resolved dependencies must be authenticated registry sources;
workspace/Git patches refuse this admission. Macro consumers still bypass.
The wrapper requires the exact vendor library source, manifest/name/version
and compiler, and refuses response files, test/compiler-extension/native-CPU
flags and unclassified dynamic macros. All other crates build without sccache.
A changed source, including injected assembler file reads, cannot inherit the
reviewed byte identity; this is not a Rust parser or generic safety claim.

This contract assumes equally trusted build producers. Read-only snapshots and
leases do not make arbitrary malicious writers to shared cache objects safe.
Cross-trust restoration needs its own producer policy and artifact custody.
Real two-revision dependency reuse, changed-revision cold-output equivalence and
actual hosted persistence/transfer cost remain acceptance work. The earlier 3%
exact-snapshot comparison must not be relabelled as this profile's speed result.
