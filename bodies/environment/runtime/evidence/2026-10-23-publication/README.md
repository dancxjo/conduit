# Committed publication CPU evidence

The candidate starts from dev after #6314. Focused controls cover canonical input
validation and all four publication contracts, commit visibility and ordering,
malformed/wrong-typed input, event/fuel capacity, cancellation/drop, zero-event
admission and full quantity coordinate width. Execution/commit/control paths
allocate and reallocate nothing after preparation. All 29 owning-runtime tests
and strict all-target lint pass. Supported candidate preflight passes workflow
lint, standalone lock checks and 345 repository CI-tool checks.

This is finite in-memory publication and CPU conformance. Leaf semantics remain
with their owning codecs; publication validates the complete canonical envelope
and exact Type. These artifacts establish no sensor readiness, Body start,
network delivery, storage authority, durable recording or overnight operation.
The existing device-protocol check includes the focused publication controls.
