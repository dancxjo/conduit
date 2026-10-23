# Native task admission

The target build instantiates the concrete Embassy task owning the ordinary
kernel and actual native provider types. The host proof now checks its ready
acknowledgment and zero-work state, rather than treating future sizing as task
admission. The portable proof covers worker scheduling, cancellation and finite
ownership independently of target task registration.

These logs prove compilation and CPU conformance, not physical task custody,
Body start, sensor dispatch or overnight recording. Target warnings identify
release and projection interfaces awaiting their separate lifecycle consumers.

The first physical image flashed successfully but failed startup. A diagnostic
reset captured a CPU stack-guard panic during canonical Source type decoding.
The linked CPU stack span had fallen to 20,788 bytes as concrete task pools and
projection statics consumed ordinary DRAM. No owner exchange or execution release
occurred. The projection's fixed 4096-byte backing now occupies separate reclaimed
DRAM; only its initialized prefix is accessed under the status mutex. The revised
image links with a 24,892-byte CPU stack span. A native-only linker assertion
refuses future images leaving less than 24 KiB; the hardware guard remains enabled.
This floor is not a worst-case stack proof for arbitrary future programs.
