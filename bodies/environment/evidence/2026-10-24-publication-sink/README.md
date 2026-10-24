# Backpressure-aware consumption of committed native publications

The portable PublicationSink contract polls acceptance of one exact committed
snapshot. Pending registers readiness and prevents another kernel Step while
the snapshot remains borrowed. Already dispatched native work may complete into
the existing bounded reply queues. The scoped stop future remains cancellable;
sink refusal follows normal quiescence, revocation and retirement. The delivery
watermark advances only after acceptance. Existing synchronous projections use
an immediate adapter with their previous acceptance semantics.

The target worker's CPU proof now covers nineteen sampler modes through both
contiguous and borrowed-record assignment projections. New modes cover a paused
recorder, cancellation while paused, explicit refusal and abandoned invocation
fencing. The successful sink stages all 960 checked source publications in
fifteen preallocated chunks, then recovers exact ordered sequences, explicit CPU
producer-clock coordinates and first-class observations. Play allocates and
reallocates nothing. No Mask page or Face-clock revision is used as a sample.

All seventeen Root tests and ten appliance tests pass. The sink invocation
future is 3,648 bytes on the measured 64-bit CPU; that excludes the external
sink's inline/heap backing and enclosing task pool. Those fifteen staging
buffers are a hosted CPU fixture, not an MCU allocation claim. Existing sampler
software preparation remains 48,483 retained bytes and 86,750 peak bytes in its
CPU fixture. Strict library lint passes; the Root harness retains its existing
allowance for included target items unused by that harness. The ESP32 native
Face release image links successfully with thirteen existing warnings.

Acceptance here is bounded in-memory ingestion, not a durable store receipt.
This does not grant storage authority, release mandatory kernel signs, establish
a network Line or configure the live Root with a recorder sink. A cancelled
pending publication receives no invented acceptance. Source clock/context must
be retained independently; receipt time is not substituted for producer time.
The updated worker's physical memory high water remains unmeasured; previous
hardware captures retain their exact earlier source identities. No firmware was
flashed for this change.
