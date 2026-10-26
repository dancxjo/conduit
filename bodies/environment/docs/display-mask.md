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
Fore metadata retain 21,491 bytes, peaking at 32,426 during cold preparation.
Play allocates nothing. Tests cover held egress, exact output bytes, cancellation
at multiple phases, renderer failure without Show, and complete value release.

These figures use 64 ordinary and 12 remote evidence slots; the earlier shared
64/64 backing retained 22,739 bytes and peaked at 33,674.

The Backs explicitly support the existing prepared-session owner. Reuse preserves
renderer RequestId and output sequence generations, accepts no reset of current
custody, and transfers the preceding invocation's exact signs. Four successive
Play identities are tested without allocations inside Play/rearm; each fresh
bounded evidence log is reserved before that measurement. Read-only interaction
emits no values and closes normally. This is not MCU session admission.

Before separating remote backing, an internal Xtensa ABI probe measured a
6,128-byte scheduler
and a 748-byte prepared wrapper (which includes its pointer, not the pointed-to
scheduler). The corresponding four-node electrical-output scheduler was
6,232 bytes.
The probe intentionally used rejected constant-array sizes to expose compiler
layouts; it did not produce, flash or validate a new running image. Even the
new scheduler plus 16,384 payload bytes exceeds the current image's 19,168 bytes
of Root headroom, before slot metadata, remote signs and renderer context.

Consequently, the ordinary Mask must replace the current output realization or
share admitted storage with it; simply adding a second prepared subsystem cannot
fit. Target preparation must consume the existing compact assigned records rather
than retain the full rich Plan/Show models. The running sampler and physical
output checkpoint remain unchanged while that admission work is incomplete.

## Compact assignment and direct value encoding

`PreparedAssignedMaskKernel` consumes the existing schema-7 assigned records and
independently supplied Make pins. It retains no rich Plan, Source graph, offer
catalogue or decoded configuration. The pins include all three label records;
packet checksums do not replace those pins. Expected Host, Boot, Plan and
fragment identities come from the current authenticated installation. Typed Fore
ABI pins and finite queue/call bounds remain checked. The constructor creates no
native possession, membership, current-offer truth or permission to start Play.

The CPU data-only fixture has a 1,989-byte assignment and 37 pinned records.
Preparation retains and peaks at 21,432 heap bytes, exactly matching
`required_heap_bytes`: the boxed scheduler and two canonical value slots. The
returned wrapper, caller stack, allocator overhead, retained wire, renderer
context and native providers are separate costs. The fixture has no Base or
resource attachment and returns a complete canonical Prepared Show only.

The generic assigned header's runtime-state ceiling is a conservative estimate
for separate per-cord buffers (49,877 bytes in this fixture). It is not this
realization's measured heap allocation or a complete target admission ledger.
This profile instead proves two shared values, including pending call and fan-out
custody, and reserves their exact backing separately. It must not be admitted by
silently treating that header as the whole appliance's available RAM.

The hosted value store can now reserve a finite slot and encode directly into it.
Remote ingress and Host Call completions use the same scheduler ownership and
sequence path as copied bytes. No intermediate Face/Show serialization buffer is
required. The full ceiling is reserved before the encoder runs. Pressure or
admission refusal does not invoke it; an encoder failure, excessive reported
length or unwind wipes and releases its private reservation. Failed attempts
consume a slot generation and cannot revive stale references. Failed ingress
publishes no value, sign or sequence. Session encoding errors retain the existing
non-rearmable failure semantics. Canonical Type admission remains the caller's
responsibility, exactly as for copied values.

The ordinary Mask's reuse proof now encodes directly into its value slots and
checks complete canonical Show bytes for changing Play identities. It also fits
each retired trace in the existing finite evidence-page codec. Separate tests
cover pressure before encoding, failure rollback, cancellation and full release.
The physical display uses the same admitted six remote evidence records as
before; its fixed log now reserves six physical remote slots instead of 64.
That changes storage, not evidence retention or delivery authority. The candidate
must pass the pinned target build and package guard before deployment; no native
Mask execution is claimed by these CPU proofs.


The updated Xtensa layout probe measures the ordinary Mask scheduler at 4,880
bytes and its compact assigned wrapper at 248 bytes. The physical-output
scheduler is now 4,840 bytes, reclaiming 1,392 bytes from its former backing.
The pinned physical-check candidate builds and passes the 165-package/25-reviewed
Conduit package guard, with the local indexed planner present and Source
parsing/checking/expansion absent. It remains unflashed; the live checkpoint still
uses the preceding image. Layout reduction is not a measured live-heap result.

Native work remains: initialize and bind the actual renderer provider under the
Body's Mask admission, preserve per-Play/effect and Show lifecycle correlation,
construct Shows without retaining rich Plan models, and replace the old output
realization inside the combined target memory budget. These are prerequisites
for claiming ordinary Mask delivery on the MCU, not optional follow-ups.
