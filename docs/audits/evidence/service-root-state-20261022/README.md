# Root register and IRQ envelope evidence

Measured clean source: `6582ce15f699ef415a65af58a4c3603b65ed4d28`. Full supported ordinary-domain-proof session 10156 passed, together with final owning fixture5 and reader2 tests. This separate evidence-only descendant preserves measured source delta objects without edits or repeated tests.

Actual x86_64 CPL3 raw trace contains original20 baseline rows/14setup records,20paired CR3 records and20paired Root-state records. Seven receipts share exact source/ELF/raw-trace identities. Actual vector7 return263, timer, fault, Root/sibling and floating controls remain in raw trace/receipts. Proof-disabled actual assembly object equals the original4056-byte extraction object. ELF/ISO remain in `work/6183-state-native-artifacts/x86_64`; final host executables preserved in `work/6183-state-proof-executables`.

Isolated proof0/1 sources/objects and snapshot/reader sources, executables and logs are supporting nonowning evidence, distinguished from final seven owning tests. The retained PCID evaluation explicitly belongs to prior CR3 source56623, not this source; the new actual CPUID header remains separately retained and its ECX bit17 independently verifies false (PCID unavailable in this measured profile too).

Six callee-saved pushes and six pops exclude flags and stack-switch cost. User-origin Root IRQ service envelope includes argument setup, stack alignment and stack-anchor restoration around the Rust call. Counts must exactly match backend user-interrupt deltas; finite aggregates refuse regression/overflow. Root-CS interrupts are excluded. Empty controls retain overhead without subtraction.

No pure trap or pure TLB cost, total IRQ/context preservation cost, calibrated timing, performance gain, normal product journey, new guest authority or issue closure is established. Budgets/profiles/deadlines remain unchanged. Logs and seven receipts are losslessly compressed with hashes.
