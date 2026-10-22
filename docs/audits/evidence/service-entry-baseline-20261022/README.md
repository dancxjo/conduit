# First service-entry matrix (#6183)

Clean measured source `a04beed4a44405fe3a42369d67afd8d9183c4cf5` passed the supported `cargo +stable xtask make conduitos ordinary-domain-proof --locked` entrance. The architecture appliance executed on QEMU 10.2.1, q35/max, one vCPU, 256 MiB. The actual ELF is `3db7d4fec1280ce7e641f15a92b02b9a20d9cd5174270d2d341911c4e7e7dddb`; the ISO is `bc1108e009bdb85def300e6efd196726c605aa7f9e0186c0d5515d24f9825a0b`.

[The receipt](positive/service-baseline-proof.json) retains all 20 observations, exact arguments, CPUID facts, build/serial identities and source. This source includes the pending #6231 fault repair and instrumentation atop development `a5600154`; it is not a claim that either pending change is accepted. Documentation added afterward does not replace the measured identity.

## Observations and accounting

These are **raw TSC counts**, with `invariant_tsc=false`, `calibrated=false`, and `component_attribution=false`. Adjacent counter pairs measured 17,310 / 10,762 / 8,832 counts; none is subtracted. Three fresh instances and three entries into one reused instance run for each normal case. Reused trial zero is that instance's first entry, not an already warm entry.

| Request | Fresh entry trials | Reused instance, first entry | Same instance, next two entries |
| --- | --- | --- | --- |
| terminal no-op | 389580 / 419710 / 230586 | 215842 | 75756 / 45064 |
| 8-byte uppercase | 978988 / 866008 / 813614 | 829918 | 57506 / 49616 |
| 256-byte uppercase | 901832 / 861328 / 852414 | 847112 | 104410 / 97804 |

Entry includes budget/context setup, guest work, interrupts and automatic fault quarantine. Separate setup, request clearing/staging, response validation/copy and teardown fields remain in every raw row. Reused installation is charged to trial zero and teardown to trial two; zero fields on other rows are accounting boundaries. The 256-byte maximum is this TextFrame fixture's capacity, not the platform transfer-window maximum.

The loop row records 16 interrupt entries and one hard preemption. The fault row is terminal. Both also retain setup and teardown; the fault entry already contains automatic quarantine, so its later idempotent teardown is not an independent subtraction. Differences between fresh/reused entries do not independently attribute trap, CR3 or TLB costs, or demonstrate a software optimization.

## Proof custody

Five receipts share the exact build and original serial digest: baseline, 26-case private service-memory fixture, guest compositor, hostile broker, and copy recovery. The full command also completes ordinary text/Source, timer, x87 vector-7/terminal/Root+sibling controls. These are emulator/fixture results, not production compositor, physical/DMA isolation, normal graphical journey or stable-release acceptance.

[manifest.json](manifest.json) records every stored/raw byte count and SHA-256, plus the locally retained ELF/ISO paths and compiler observation. All 33 copied records were rehashed; every gzip round-trips to its original bytes. The original serial contains binary presentation bytes, including NULs, and is preserved without normalization. The ELF and ISO remain outside disposable caches and are excluded from Git. Build-side privilege inventory remains link observation with its own false isolation flags.

Focused real formatter and strict complete-line reader tests each pass two cases. Whole-workspace formatting failed on two inherited development formatting defects; that negative is retained, and #6252 separately repairs it. The three earlier boot/admission negatives remain source-bound under `negative/`: 128 MiB runtime arena refusal, 256 MiB ordinary admission refusal, and the exact text-binding diagnostic. Earlier compiler failures also remain in the recorder's original evidence directory.

## Remaining #6183 acceptance

The issue remains open for larger shared-window/staged-IPC sweeps, independent component attribution and overhead controls, a demonstrated optimization with paired before/after protection equivalence, and the normal responsive journey comparison. No PCID change or calibrated/physical performance claim follows from this first matrix. Candidate and relevant merged Integration for delivery remain separate gates.
