# Prepared compact output ownership

Development work for #6099. The profile is not an installed Std/Body numerical
Fore, a published release, or a claim that all numerical copying is removed.

The existing copied compact DAG remains an explicit baseline.
`FixedScheduler::bind_owned_compact_f32_dag` instead selects ownership transfer
for internal producers that expose their exact prepared raw f32 buffer.
CompactNumericBack and CompactLinearBack provide this contract. Other producers
remain outside that ownership contract. Once selected, an owned publication
cannot silently fall back to copying.

Preparation reserves typed output pools before Play. Atomic PreserveOrder
fan-out shares one ValueRef, so the largest outgoing queue capacity plus one
staging slot bounds each producer pool. Separate free canonical slots cover
other queued values and canonical staging. The concrete caller must supply any
additional retained-input and Host-completion requirements. Admission excludes
live values, retired generations and earlier typed pools, and matches remaining
slot capacities without converting canonical capacity into raw-only storage.

Opaque producer/store identities and preallocated receipt markers bind each
pending transfer. Materialization swaps the completed buffer into an admitted
slot. The producer receives an empty reserved buffer; rollback returns its
original contents. Commit preserves ordinary shared-reference fan-out, quotas
and monotonic generations. Losing an unresolved receipt retires its slot rather
than allocating a replacement during Play.

## FARGAN worker profile

The supported development entry accepts the same explicit private pinned fixture
as the copied profile:

```sh
cargo xtask prove runtime-performance \
  --fargan-fixture /path/to/private/fixture \
  --fargan-owners prepared-owned-matrix-local \
  --repetitions 2 --output /path/to/new/capture
```

Use `prepared-matrix-local` for the copied baseline. The ownership profile uses
the same checked Source and planned matrix/local-expression realization. Its
concrete preparation refuses Host Calls. Resource access remains with the exact
existing admitted matrix owner. The worker reports copied publication bytes
separately from transferred publication bytes; these counters do not count all
arithmetic, codec or tensor memory traffic.

The ordinary real-provider tests compare copied and owned pointwise execution
at 40, 128 and 160 lanes, including overflow, output/event equality and actual
publication counters. An explicit private-fixture diagnostic additionally
compares two invocations of the same real FARGAN frame with fresh Play identities
and checks that Play performs no allocations. That diagnostic is distinct from
matched release observations of complete preparation, rearm, output acceptance
and disposal.

Full FARGAN adoption, cancellation/failed-acceptance frontier controls and
matched full-owner measurements must pass before #6099 can close. No portable
speedup or complete-copy-elimination claim follows from pointer transfer alone.
