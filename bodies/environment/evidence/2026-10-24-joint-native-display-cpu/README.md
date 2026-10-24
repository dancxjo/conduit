# Joint native worker, Face and checked display (CPU)

The actual native sampler worker publishes its first observation into retained
local Face custody. The prepared Fahrenheit Look and alternating temperature/RH
Mask produce two owned Shows from that same Face revision. The existing compact
assigned output kernel and target display service send actual selected Host Calls
to the existing TM1637 owner. Native GPIO, authority, identity and clock remain
explicit CPU fixtures. This proves no physical output or Body membership.

The normal case acknowledges both writes, drains output, then accepts the
remaining sampler publications and retires the actual cleaned-up providers.
The cancellation case cancels the electrical write before START, preserves the
original `Cancelled/13` failure through the packet Back and publication sink,
quiesces the sampler and retires its revoked providers. No write is acknowledged.
Both contiguous and record-based Source assignments exercise these cases.

## Memory scope

`measurement.json` records requested heap bytes on this 64-bit CPU. Source
preparation is observed first, retaining its actual kernel. The second observation
retains the output kernel, Face, Mask, native worker box, selected pin owner and
CPU pin-counter owner. Output software reconstruction, generated assignment and
layout material are included in this second preparation peak; temporary material
is released before Play. The two observations do not free pre-existing ownership.
The source-first peak is `max(source_peak, source_retained + additional_peak)`;
retained overlap is `source_retained + additional_retained`. These are measured
ownership lifetimes from one joint construction, not a sum of unrelated HIL peaks.

Excluded: the pre-existing Source/Plan and resident descriptions, output Source,
checked Plan and trusted Make metadata, already-bound pulse/timer providers,
Root command/Body/HTTP/network owners, allocator overhead, stack, OS and hardware
resources. CPU generated assignment preparation is not MCU linked-material cost.
Zero allocations/reallocations covers the complete native worker execution with
both output pages and the cancellation path. It does not cover preparation.

This is a finite two-frame output proof; only the first sampler Face is displayed.
It does not admit an overnight display loop, evidence handoff, network transition
while sampling, or a combined MCU profile. MCU memory/stack admission remains open.

## Reproduce

`cargo xtask prove environment native-construction`

The joint cases run inside the independent finite pulse/wait Root authority test.
`cargo xtask check tm1637-output` also covers the packet Back's cancellation
completion contract: absent detail remains `Cancelled/0`, a supplied cancellation
reason is preserved, and an inconsistent supplied reason is refused.
