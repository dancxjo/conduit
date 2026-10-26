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
