# Native application service

A separate Application-profile owner holds the retained canonical native
Tutorial, Tour and Patchbay controllers for one NativeWorkset. Root's Runtime
service remains the sole native-workset scheduler and delivers exact Host Call
inputs to this owner through Root's admitted value-loan boundary.

The independently linked guest has a 2 MiB image, a 4 KiB exchange, a 16 KiB
transfer window, a 1 MiB stack and 2 MiB private metadata. Linked mutable data
occupies only the first metadata page; State occupies the next page. A private
bitmap allocator uses the remaining admitted metadata after its 24 KiB header.
Compile-time assertions bound State and allocator metadata. They do not establish
peak stack fit, useful heap headroom, execution timing or runtime acceptance.

Faults and protocol failures are terminal: Root releases the lease and fences
pending effects and stale outputs without retry. This source migration does not
move Body lifecycle authority, the Face/Mask pipeline, physical drivers, networking
or other Root runtime paths out of privilege. No QEMU, physical, CI or release
acceptance is claimed by these sources.

Lifecycle Tutorial projection reuses a live Application owner through a read-only
Root transport borrow. A cold/Lulled journey or text-only workset uses one explicit
transient lease that is dropped after the query, leaving the slot available for
Wake. Failure of a live owner never triggers a transient fallback. Root action
admission and lifecycle effects remain privileged. A failed read fences all cached
NativeWorkset views and action getters immediately; the next mutable boundary
retires the sibling Runtime/text ownership.
