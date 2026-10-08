# Synthetic numeric ConduitOS guest

This optional development proof executes the checked four-subframe signal epoch
with synthetic tensors on booted x86_64 ConduitOS under QEMU. It does not admit a
pretrained model, linguistic intent, shared speech IR, or voice. The conditioner,
utterance feedback loop, trained PCM differential and real-time acceptance are
outside this proof.

Export local materials with the ignored
`export_synthetic_epoch_guest_preparation_materials` test in
`semantics/ai/tests/fargan_epoch_flow.rs`, selecting an empty directory through
`CONDUIT_NUMERIC_GUEST_FIXTURE`. No model download is performed. Then run:

```sh
proof/fargan/build_synthetic_guest.sh FIXTURE_DIRECTORY FRESH_IMAGE_DIRECTORY PINNED_LIMINE_ARCHIVE
proof/fargan/run_synthetic_guest.sh FRESH_IMAGE_DIRECTORY/conduitos.iso FRESH_TRANSCRIPT_DIRECTORY
```

The build verifies the pinned Limine archive, builds its installer locally, and
uses a distinct preparation/Make profile. Existing default profiles are unchanged.
The guest rechecks Source, creates a fresh Boot-bound Plan, admits immutable
synthetic resources, prepares the existing portable operation owners, and seals
the allocator before scheduler execution. Acceptance requires full drain, one
output, unchanged allocation-request and live-byte counts, the serial PASS marker,
and QEMU debug-exit status33. Keep scripts unchanged during an active run.

Observed release opt1/codegen16 execution used 770 nodes, 1,217 cords, 26 tensor
resources (2,989,684 raw bytes), and 27 ingress values. The guest drained in 3,123
scheduler steps with 218 expression calls, 66 peak fixed cells and 61,415 peak
payload bytes. Its 12,246-byte canonical output SHA256 was
`898ff8d4a79bf03a54d8ea900d534b20be4af8931d4d4fa5ce3d833cad725908`,
identical to hosted execution of the same checked topology.

The 256MiB arena peaked at 207,073,120 bytes during preparation and retained
63,790,528 bytes through execution. Allocation requests remained 23,497,102 before
and after execution; post-seal requests were zero. The caller-placed fixed store
occupies 16,785,424 bytes. ELF sections measured text38,608,419, data16,788,976 and
BSS1,048,728 bytes; the fixed store is in data. The requested stack is 64MiB,
which is a reservation, not a measured stack high-water mark. These quantities
must not be added as though they are all independent heap allocations.

This is one synthetic epoch on an emulator, with a large explicit preparation
arena and retained schema/Plan material. It establishes neither fixed small-memory
product fit nor execution latency, throughput, physical-target, or speech quality
acceptance. Preserve complete build, serial, command, image-digest and terminal
receipts when reporting a particular run.
