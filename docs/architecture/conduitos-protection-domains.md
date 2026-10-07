# ConduitOS protection domains

Status: one bounded freestanding-emulator enforcement profile, owned by
[#3078](https://github.com/dancxjo/conduit/issues/3078). This is not a claim
about every ConduitOS architecture, hostile kernel providers, or DMA isolation.

## Enforced boundary

The first profile executes its adversarial implementation at x86 privilege
ring 3. A dedicated four-level page table gives that domain one read-only code
page, one writable private-data page, and one writable stack page. Kernel text,
kernel data, the kernel capability table, the trap stack, sibling-domain
addresses, and device MMIO have no user-accessible page-table entries. The TSS
owns the ring-0 trap stack and its I/O bitmap ends at the TSS boundary, so user
I/O instructions fault rather than reaching a port.

The trusted computing base for this profile is the ConduitOS x86_64 bootstrap,
GDT/TSS and IDT setup, page-table construction, trap assembly, kernel
capability table, serial base provider, Limine, QEMU's emulated processor and
UART, and the repository proof harness. The provider remains trusted kernel
code. The implementation receives no kernel or device pointer.

## Capability gate

`protection_domain.rs` is the architecture-neutral, allocation-free kernel
table. Each entry binds a protection-domain identity to the exact host, boot,
plan, play, implementation, base and generation, resource and generation,
operation, subject, authority, parameter/work envelope, operation count, and
in-flight bound. An untrusted integer is accepted only when it matches a live
kernel entry owned by the calling domain. Completion leases carry the table
generation, so revocation fences in-flight completion as well as later calls.

Cancellation, completion, plan replacement, authority revocation, resource or
base replacement, and boot replacement all enter the same kernel-owned
revocation transition with a distinct machine-readable cause. Semantic state
may use its own typed continuity mechanism; handles never cross that boundary.

## Executable proof

Run:

```text
cargo xtask make conduitos isolation-proof
```

The harness builds and boots the actual `x86_64-unknown-none` image. The same
ring-3 fixture performs a pure checkpoint with no handle, then attempts kernel
memory, a write through the actual kernel capability-table address, a read
through the actual sibling-state address, an arbitrary kernel-function call,
device-MMIO space, direct serial-port I/O, an arbitrary handle, another
domain's real handle, a wrong operation, excess submission, a malformed
pointer, and a revoked handle. It also performs one
authorized serial presentation through the capability gate. The kernel resumes
the bounded fixture after expected protection faults and terminates on any
unexpected vector or sequence.

Acceptance requires both the structured protection sign and the independently
observed `CONDUIT_SERIAL_PRESENT protected-domain-authorized-effect` line. The
sibling sentinel is independently checked by the kernel before success. A
protection fault is recorded as a refusal fact, never semantic completion.

## Ordinary execution work in progress (#5113)

`protected_region.rs` owns verified Plan/Play/region/domain binding, finite
memory admission, and terminal lifecycle transitions independently of the CPU
backend. Protection faults and provider loss revoke domain handles and fence
in-flight leases. `authorize_current` compares the complete currently selected
Root scope before accepting a gate claim, including Host, implementation,
Base/resource, subject, authority and their admitted envelopes.

The x86_64 ordinary three-placement text runner now loads an independently
compiled Unicode-uppercase implementation into a CPL3 address space. It uses
one entry and one terminal gate for the entire bounded input rather than a
privileged transition per character. Code is read-only/executable; constants,
input/output and stack have separate permissions; inherited Root mappings are
supervisor-only. Floating-point access traps instead of exposing Root register
state. A hard timer returns a hostile loop as a work-exhaustion fault. A fault
terminates the region; it does not resume the hostile instruction.

The supplemental `cargo xtask make conduitos ordinary-domain-proof` lane runs
checked ordinary text Source through the production kernel and then checks 17
hostile entries: Root memory, capability memory, sibling memory, Root entry,
MMIO, ports, interrupt disabling, an infinite loop, floating-point access,
alternate syscall entries, division by zero, breakpoint, single-step, timestamp
access, code writes, and data execution. This lane does not establish that every
current product path uses protection domains.

The text region requests one serial presentation through an opaque authenticated
handle. Root checks the current Host, Boot, Plan, Play, selected implementation,
provider and resource generations, operation, subject, authority and bounded
request before operating its serial Base. Successful completion consumes the
one-operation grant. Provider loss and generation changes fail the operation.

Supplemental hostile-image fixtures use fresh Play identities and the same
production gate. The local emulator refused unknown and stolen sibling handles,
wrong operations, oversized windows, excessive work, invalid capacities and
UTF-8, and forged hardware-fault results before operating the Base. Replay and
operation exhaustion preserved a one-effect count. Revoked lifecycle states
performed no effect; provider loss revoked the grant. Fixture cost records are
marked separately from the ordinary Source record. These fixtures supplement
the actual Source run; they do not establish normal graphical-path integration.

The local positive x86_64 QEMU run produced `HELLO, CONDUITOS` through that gate.
It recorded three entries, three gates, one Base gate, six CR3 switches/TLB
flushes, three scheduler returns, 64 runtime copy bytes, a 32-byte shared-window
peak, and 118,784 backend bytes. Setup image copies and setup/teardown TSC ticks
are reported separately. Root metadata is charged to region admission; teardown
zeros the backend allocation. These are emulator measurements, with a shared
page and no ring slots; scheduler/timer coexistence remains unfinished.

| Target | Ordinary protection evidence | Remaining boundary |
|---|---|---|
| x86_64 | Supplemental emulator run of checked text Source and hostile entries; bounded CPL3 implementation in the ordinary text runner | Current product paths and shared timer reservation remain unfinished |
| IA-32 | ELF32 artifact admission and malformed-mapping tests | No earned ordinary CPL3 execution proof |
| AArch64 | Separately compiled pure image only | No earned ordinary EL0 execution proof |
| RISC-V64 | Separately compiled pure image only | No earned ordinary U-mode execution proof |
| LoongArch64 | Separately compiled pure image only | No earned ordinary least-privileged execution proof |
| ARMv6 | No earned protected backend | Protected execution is unsupported; cooperative execution is not confinement |

The ordinary text serial presentation now passes through its domain capability
gate, and the supplemental Sign reports `effect_capability_gates:true`.
The current native workset, timer/Morse and other paths have not been migrated.
DMA and driver isolation remain false.
Issue #5113 stays open until its complete cross-architecture product and proof
criteria are earned; the local emulator run is not accepted-release evidence.

## Deliberate limits

This profile proves CPU page, privilege, I/O-port, handle, and one serial base
boundary on emulated x86_64. It does not prove an IOMMU, DMA containment,
mutually isolated kernel drivers/providers, physical hardware execution, SMP,
or any non-x86_64 target. The sign reports `dma_isolation:false` and
`driver_isolation:false`. Follow-on architecture profiles must earn their own
mechanism-level evidence; none inherit this proof by analogy.
