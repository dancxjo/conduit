# RP2040 snapshot NOR provider

This adapter consumes Embassy's sole blocking FLASH owner and implements the
shared immutable `NorStorage` interface within a checked subslot of `DURABLE`.
It programs exact 256-byte pages from retained RAM, bounds every operation, and
exposes no erase. It neither chooses free generations nor publishes a Ready Base.

The pinned Embassy implementation pauses core 1, excludes interrupts and waits
for XIP DMA before executing flash operations from RAM. This provides physical
execution safety, not a Conduit grant or a wall-clock deadline. Root must fund
the provider/buffer and blocking execution window, exclude overlap with
in-flight timing-sensitive GPIO work, bind exact current native
capability and full-workset admission/Start, and retain ownership until requests
are quiescent. A physical program failure poisons further programming. The shared
snapshot publisher performs readback and the separate publication marker.

`Slot` can be tested without hardware. Feature `rp2040` enables the physical
provider; it has not yet been connected or power-cut qualified on Yellow.
