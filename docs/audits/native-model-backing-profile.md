# Native CPU backing profile (#6208)

The optional `conduit-std-host/native-model-backing` feature prepares one exact
Linux x86_64 glibc CPU profile: ORT 1.23.2 with the reviewed runtime SHA-256,
F32 scalar Identity graph with the reviewed 77-byte SHA-256, sequential execution,
one intra/inter-op thread, disabled graph optimizations, CPU arena and memory
patterns. Unsupported profiles refuse before a prepared resource is returned.
The parent retains the admitted canonical model resource and its read custody.
This is the native storage admission seam; the ordinary Source/Plan/Play adapter
and broader resource lifecycle remain separate work.

Preparation obtains actual private mappings, write-prefaults every writable
private page, and installs a process-wide seccomp allowlist. Mapping/heap growth,
discard, protection changes, new threads and processes terminate the worker.
The original startup stack has a worker-local hard limit equal to its mapped
extent; execution uses a fixed guarded stack. No pinned RAM or physical residency
claim is made. Paging existing backing is permitted.

The parent independently reads the waiting child's kernel mapping inventory
before returning the prepared resource. Private capacity is partitioned into
fixed I/O, execution stack and one shared native pool. Session/metadata and
execution scratch are explicit aliases of that pool, charged once. This pool
capacity is authoritative reserved storage, not an estimated per-call maximum.
All image mapping address reservations are charged, including read-only private
ELF/RELRO pages that may have relocated copies. Deduplicated source file extents
are an inspection inventory, never an additional charge or a claim that relocated
pages share storage. Other fixed mappings, including guards and kernel mappings,
are included in the fixed mapping grant. The child's graph copy is in its private
pool; the parent's canonical model resource remains separately owned.

Each result carries a sequence and exact profile-specific numerical check before
becoming visible. A growth attempt kills the worker and returns no successful
result. The native pressure control is deliberately process-fatal. Disposable
process cleanup does not claim the broader ORT session retirement contract.

Run the supported acceptance entrance with an explicitly configured installed
runtime (the command never discovers or downloads a runtime/model):

```sh
CONDUIT_ORT_LIBRARY=/absolute/path/to/reviewed/libonnxruntime.so.1.23.2 \
  cargo +stable xtask check model-native-backing
```

It runs owning unit tests, 1,000 exact CPU outputs, unchanged mapping verification,
insufficient-grant refusal before readiness, and forced-capacity failure with no
returned value. CI's std shard explicitly compiles and tests this optional
feature; actual CPU acceptance needs the configured installed runtime. Default
and no_std dependency closures do not include the native dependency. `ort-sys`
uses API 23 and `disable-linking`, without runtime binary download features.
