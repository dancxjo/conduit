# Evictable blob cache

## Boundary

`storage/cache/put`, `storage/cache/get`, and `storage/cache/remove` are
optional host boundaries for one best-effort, evictable, content-addressed
cache. `storage/blob/literal` is a finite fixture source. These contracts do
not imply durable storage, implicit placement, replication, migration, or
future provider availability.

The exact value identities are:

- `storage/blob`: one bounded byte chunk;
- `storage/blob-identity`: SHA-256 content digest plus exact byte length;
- `storage/cache-handle`: an opaque provider- and run-scoped reference;
- operation-specific put, get, and remove results.

A handle is neither a grant nor proof that bytes remain present.

## Provider description and admission

A provider description names its immutable identity and bounds for blob size,
total retained bytes, entry count, pending operations, retention, timers,
work, sensitivity, integrity, eviction, evidence, and availability class.
The current reference profile is `best-effort`, `evictable`, SHA-256 checked,
and deterministic FIFO. A durable requirement, excessive object, refused
sensitivity, insufficient capacity, stale observation, unavailable provider,
or missing grant fails before mutation.

The exact plan separately pins the semantic contract, implementation,
artifact, host observation, provider description, cache resource, grant,
budgets, and operation configuration. Discovery and inspection never allocate
cache storage.

## Operations

`put` receives one bounded blob and emits an opaque handle plus a commit
result. It records any deterministic eviction and an upper retention tick.
The retention tick is not a durability promise.

`get` receives a handle and emits a bounded blob plus an exact outcome:
`hit`, `miss`, `evicted`, or `expired`. A hit recomputes the content digest
before yielding bytes. Provider loss and integrity failure are terminal
failures rather than misses.

`remove` receives a handle and emits `removed` or `missing`. Removal is
idempotent only at this operation boundary; it does not widen authority.

All operations have finite input/output values, one pending-operation
ceiling, explicit cancellation, and bounded evidence. There is no hidden
fallback or provider reselection after start.

## Reference conformance

The allocator-free oracle in `conduit-std` covers put/hit/miss/remove,
FIFO eviction, expiry, provider loss, durable-requirement rejection,
oversize, sensitivity refusal, digest mismatch, wrong provider/run handles,
and cancellation. Hosted providers must normalize those outcomes without
exposing provider paths, secret material, or blob bytes in evidence.
