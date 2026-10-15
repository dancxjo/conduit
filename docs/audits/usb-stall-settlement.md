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

## Refreshed product validation

At `5bcc8b9e23d6817364fa8ac2feec783da85951ee`, 38 focused hosted tests,
strict all-target Conduitos Clippy and pinned workspace formatting pass again.
The canonical `make conduitos live x86_64 --locked` product build and all ten
fresh BOT/cancellation cases pass. All ten transcripts retain zero Play
allocation/reallocation counters. Sixteen ordinary read tags and one typed
retired STALL result are checked. External reply after cancellation still does
not prove late native completion. The original receipts above remain pinned.

An initial architecture-appliance build was rejected by the product-role guard
before any guest case; it is excluded. The harness can orphan emulator/device
children when its wrapper terminates; only descendants of already verified
cases were cleaned up, preserving active cases and proof files. These hosted
fixture process lifetimes do not prove or change native DMA quiescence.

The subsequent dependency refresh adds the PC repair's two exact Host profile
snapshot literals, with no additional product-code changes.

The BOT wrapper now installs Linux parent-death termination for both its actual
QEMU and external device children, including the fork/installation race. The
same actual-QEMU supervision regression fails on the original wrapper and
passes on the guarded wrapper. It checks forced wrapper death with QEMU paused;
it is host probe ownership proof, not native transfer or DMA evidence. The ten
USB guest receipts above remain pinned to their measured product Source.
