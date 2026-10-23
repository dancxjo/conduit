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

The 24,892-byte-stack image also failed startup; a reset captured an instruction
exception in an RTOS task. Therefore neither a successful task admission nor a
sufficient stack bound is claimed for that image. The next fixed partition moves
4 KiB of Root heap backing from ordinary DRAM to reclaimed DRAM: Root remains
112 KiB (60+52) and the isolated SDK remains 72 KiB (32+40). The linker floor is
raised to 28 KiB, close to the previously working image's stack span. Startup
and the complete held-worker exchange must be physically verified again.

The third image also failed startup with a CPU stack-guard write while evaluating
a Source literal. Its linked CPU stack span was 29,020 bytes, compared with
29,028 in the prior verified image; restoring the approximate span was therefore
insufficient. These partition changes are compilation evidence only. The prior
verified image is being restored while the actual preparation call-stack closure
is investigated. No Body start, native dispatch or task-ready receipt was reached.

Restoration of the prior verified ELF completed. A fresh read-only UART offer
confirmed Root responsiveness, with no Body/native plan installed and no
appliance kernel prepared in this new Boot. The old reservation from the prior
Boot is not reused. The held-task image remains unverified and is not installed.
