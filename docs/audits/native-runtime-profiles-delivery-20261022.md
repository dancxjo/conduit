# Native document and Face runtime preparation

This #6201 prerequisite reuses native-localc440e38c6's runtime-core delta only.
It prepares the existing fixed scheduler for initialized document/editor flow and
an explicitly selected Face-input profile. Root custody, semantic Back dispatch,
Host offers, production Source and UI/input routes are not switched here.

The legacy Scheduler alias keeps3072-byte values. The explicit Face profile has
8192-byte values; both retain ten value slots,24576 aggregate bytes, fourteen
nodes and ten Cords. Face admission refuses oversized contracts and remote
sinks. Its finite remote-event Sign history uses already physically present
storage and cannot replenish itself after pressure. Only one scheduler profile
occupies the service owner at a time; there is no second execution kernel.

Face initialization and typed remote admission are additive wire tags; existing
legacy tags retain their meaning. Accepted/Full sequence outcomes remain distinct
from failure. Legacy owners refuse the new ingress before mutation. The retained
Runtime has a compile-time bound against its existing private state region.

Document admission prepares the genuine revision-zero empty document before
Play. Its initial output precedes any edit Host Call; an empty sentinel is not
an edit. Later edits and committed snapshots are checked against the existing
272/264-byte profile. Semantic decoding and authoritative document state remain
with the admitted protected Back, not this transport driver.

All six owning-package hosted tests pass, covering initialization, legacy
compatibility, exact8192-byte ingress, pressure/sequence, bounds and strict wire
roundtrips. The package all-target check, freestanding x86_64 library check and
changed-Rust formatting pass. Logs are retained under
`evidence/native-runtime-profiles-delivery-20261022/`.

Hosted debug Face constructors use an explicit8 MiB fixture thread stack. Native
stack/peak fit and actual protected-service execution are not proved by that
allowance or compilation. No heap, private region, global legacy limit, product
image or platform proof is enlarged/claimed. CI is deliberately skipped per the
user's instruction. #6201/#5994 and the actual custody/caller migration stay open.
