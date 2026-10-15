# Acknowledged halted USB queue settlement

For #5586, using the accepted #5437 transport and the PC preparation repair
tracked by #5867. A fresh normal-product BOT STALL run at
`c77ea77fd3372d7b03bf62a8522970cbbb82a33d` failed with a Host-call refusal
instead of the typed stalled storage result. Actual QEMU tracing showed the
original Reset Endpoint command succeeding. Storage settlement then required
idle while the acknowledged halted queue still retained its recovery marker.
Recovery control dispatch had an entrance for the exact dequeue transition;
terminal storage settlement did not.

At `29f2607a6a4dded27bf88bfc6c44bcedefcf9f9b`, settlement checks the original
Plan/Boot, dispatcher tokens and Kernel subordinate obligations, then uses the
existing exact Set TR Dequeue path to position that retired empty queue before
checking native idle. Failure retains quarantine. The public borrowed idle
check remains read-only. This adds no class policy, transfer retry, replay or
semantic success inference. The storage association still owns its receipt.

Strict Conduitos Clippy and 38 focused tests passed: seven actual dispatcher/
association tests and 31 native-host tests. An earlier mistaken native filter
selected zero tests and is excluded. The latter suite exercises original/foreign
and failed stop/reset/dequeue acknowledgement, retained DMA, cancellation and
late original events through native event-ring code with synthetic MMIO.

The canonical normal PC image then built and all ten fresh QEMU BOT cases
passed on that exact kernel: STALL, sixteen normal reads, disconnect, stale
tag, bad residue, command failure, phase error, short input, held input cancel,
and external reply after cancellation. The STALL case published exactly one
typed stalled result and retired complete. Every transcript reported zero
Play allocations and reallocations. Exact source/kernel identities and artifact
hashes are retained in [evidence/usb-stall-settlement.json](evidence/usb-stall-settlement.json).

External replies after cancellation do not establish a late native controller
completion; their verifier explicitly reports that fact as false. Native late
event refusal is a separate hosted proof class. These are emulator and hosted
tests, not physical compatibility. #5587 and parent #5253 retain their current
full integration, architecture comparison and publication gates.
