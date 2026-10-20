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

## Retained development observation

The [observation record](../../proof/performance/numeric/compact-owned-output-20261020.json)
compares committed pre-feature source `a020695ca510` with owned source
`e992687c343a`, using archived release workers and the same private pinned
24-frame trajectory, twice. All 48 retained outputs, event logs and Plans match
by hash. Both workers pass their conformance entrance, and source custody is
verified. Private model inputs, raw captures and executables remain local.

| Per frame or prepared controller | Copied baseline | Owned profile |
| --- | ---: | ---: |
| Copied compact publication bytes per frame | 158,208 | 39,936 |
| Transferred publication bytes per frame | 0 | 118,272 |
| Owned publications per frame | 0 | 164 |
| Allocations/reallocations during Play | 0 | 0 |
| Controller heap reclaimed at final disposal | 40,170,366 B | 35,124,307 B |
| Cold controller allocation/reallocation requests | 4,365,201 | 4,368,104 |
| Cold controller cumulative requested bytes | 636,334,977 B | 638,273,254 B |
| Observed median Play wall time | 21.221 ms | 21.871 ms |

This is a copy/residency improvement with additional preparation work. Cold
allocator-observed peak live heap also increased by about 59 KiB; the complete
phase measurements retain that cost. The single sequential capture pair does
not establish a latency benefit: the owned median was slightly slower. These
heap observations include the fixture controller and common reference state;
they are not RSS or a claim about every internal copy or memory write.

The first candidate disabled the generic uniform-slot fast allocator when
reserving typed pools and had a 23.854 ms median. The corrected allocator keeps
its cursor, excludes typed slots, does not rewind it when owned buffers release,
and distinguishes generic pressure from retired owned generations. The earlier
capture remains in the observation record rather than being discarded.

Validation includes the full release kernel suite (205 unit tests, three
existing ignored, plus integration suites), 15 compact-DAG tests, and five new
owned controls for pressure/fan-out, failed acceptance rollback, Sign admission
rollback, owner denial without fallback and cancellation at every logical
frontier. The separate private-fixture diagnostic passed two repeated real
frames with exact event/output equality and zero Play allocations.

Implementation closure still requires its reviewed implementation to merge and
required current-source checks to pass. This is not a published release, an
installed Body Fore, or a portable speedup claim.
