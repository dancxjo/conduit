# Seven-segment Mask preparation

`PreparedDisplayMask` consumes an immutable environmental Face and a Look. The
Face retains first-class temperature and relative humidity. The Look chooses
Fahrenheit or Celsius and the fraction annunciator; the hardware profile maps
the finite Show to wired segment, decimal-point and colon bits.

`prepare_show` resolves temperature and humidity once into owned finite material
for that exact Face identity and revision. `render_prepared` selects a page from
an explicit elapsed value and composes the independently checked TM1637 Frame.
No Face borrow survives preparation. Selecting another page neither publishes a
new observation nor advances the Face's freshness clock. At the configured finite
lifetime boundary the presentation becomes unavailable. A newly stale or
unavailable Face resolves to the corresponding unavailable presentation.

These calls allocate no heap after the prepared frame storage exists. Segment
patterns use their checked seven-bit Form; annunciators remain separate until
hardware mapping. This representation does not replace the quantities in Face.

This is inert rendering material, not an initialized Mask role. Its owner must
admit the page clock, invocation and output bounds, retain current Face/provider
custody, replace or revoke obsolete material, and clear manifestation on loss
or cancellation. A typed Frame grants no output authority and performs no GPIO
write. The ordinary selected output Fore and its lifecycle evidence remain
required for physical delivery.

## Complete Mask wire preparation

`SevenSegmentMaskSource` checks and plans the existing ordinary Mask Source
against caller-supplied current Host offers, resources and grants. It creates no
provider possession. The CPU planning fixture deliberately supplies renderer
metadata only; it proves neither native Mask admission nor physical readiness.

The fixture retains 14,083 bytes for its ordinary Plan and another 16,300 bytes
for a prepared rich `MaskShow`. Serializing that Show directly peaks at 275,846
allocated bytes, mostly because the embedded complete Plan is compressed again.
These are CPU allocation measurements, not ESP32 memory or fit measurements.

`PreparedMaskPlanJson` prepares the complete existing Plan wire once.
`PreparedMaskShowJson` owns a snapshot of the remaining Show metadata and borrows
only that immutable wire; the rich Plan and Show can be dropped before encoding.
The same fixture retains 4,453 bytes for cached Plan wire and 2,270 bytes for the
Show snapshot. Encoding into the caller's 6,723-byte buffer allocates nothing over
960 repetitions. Prepared, Available, Closed and Failed codec fixtures match the
existing serializer byte for byte. A different complete Plan is rejected even
when its Plan ID is unchanged. Too-small output is rejected before mutation.

Preparation still allocates and compresses, and its byte ceiling bounds the
result rather than its temporary heap. It belongs off the tight execution path.
Encoding neither validates a current Face nor acknowledges delivery: ordinary
owner admission, Plan/Play correlation, lifecycle and actual output acceptance
remain required. This codec is not yet installed as the MCU's ordinary Mask.

## Ordinary shared-kernel realization

The opt-in `mask-kernel` runtime feature prepares the existing three-node
tee/renderer/read-only interaction Mask from its verified ordinary Plan. It
checks the exact port contracts and one-item bounds, lowers the routing and Host
Call, and uses the existing scheduler. It initializes no renderer or electrical
provider and grants no authority. The CPU renderer fixture returns a Prepared
Show; it cannot establish physical delivery or an Available Show.

Face fan-out shares one retained value. The renderer's pending Host Call retains
that value until completion; its result needs a second slot. Show fan-out shares
that second value between interaction closure and egress. With 8 KiB Plan
ceilings, value backing is 16,448 bytes on the CPU. The full prepared kernel and
Fore metadata retain 22,739 bytes, peaking at 33,674 during cold preparation.
Play allocates nothing. Tests cover held egress, exact output bytes, cancellation
at multiple phases, renderer failure without Show, and complete value release.

The Backs explicitly support the existing prepared-session owner. Reuse preserves
renderer RequestId and output sequence generations, accepts no reset of current
custody, and transfers the preceding invocation's exact signs. Four successive
Play identities are tested without allocations inside Play/rearm; each fresh
bounded evidence log is reserved before that measurement. Read-only interaction
emits no values and closes normally. This is not MCU session admission.

An internal Xtensa ABI probe of these exact types measured a 6,128-byte scheduler
and a 748-byte prepared wrapper (which includes its pointer, not the pointed-to
scheduler). The existing four-node electrical-output scheduler is 6,232 bytes.
The probe intentionally used rejected constant-array sizes to expose compiler
layouts; it did not produce, flash or validate a new running image. Even the
new scheduler plus 16,384 payload bytes exceeds the current image's 19,168 bytes
of Root headroom, before slot metadata, remote signs and renderer context.

Consequently, the ordinary Mask must replace the current output realization or
share admitted storage with it; simply adding a second prepared subsystem cannot
fit. Target preparation must consume the existing compact assigned records rather
than retain the full rich Plan/Show models. The running sampler and physical
output checkpoint remain unchanged while that admission work is incomplete.
