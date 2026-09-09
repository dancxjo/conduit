# ConduitOS x86_64 protection domains

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
capability table, serial Base provider, Limine, QEMU's emulated processor and
UART, and the repository proof harness. The provider remains trusted kernel
code. The implementation receives no kernel or device pointer.

## Capability gate

`protection_domain.rs` is the architecture-neutral, allocation-free kernel
table. Each entry binds a protection-domain identity to the exact Host, Boot,
Plan, Play, implementation, Base and generation, Resource and generation,
operation, subject, authority, parameter/work envelope, operation count, and
in-flight bound. An untrusted integer is accepted only when it matches a live
kernel entry owned by the calling domain. Completion leases carry the table
generation, so revocation fences in-flight completion as well as later calls.

Cancellation, completion, Plan replacement, authority revocation, Resource or
Base replacement, and Boot replacement all enter the same kernel-owned
revocation transition with a distinct machine-readable cause. Semantic state
may use its own typed continuity mechanism; handles never cross that boundary.

## Executable proof

Run:

```text
cargo xtask conduitos isolation-proof
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

Acceptance requires both the structured protection Sign and the independently
observed `CONDUIT_SERIAL_PRESENT protected-domain-authorized-effect` line. The
sibling sentinel is independently checked by the kernel before success. A
protection fault is recorded as a refusal fact, never semantic completion.

## Deliberate limits

This profile proves CPU page, privilege, I/O-port, handle, and one serial Base
boundary on emulated x86_64. It does not prove an IOMMU, DMA containment,
mutually isolated kernel drivers/providers, physical hardware execution, SMP,
or any non-x86_64 target. The Sign reports `dma_isolation:false` and
`driver_isolation:false`. Follow-on architecture profiles must earn their own
mechanism-level evidence; none inherit this proof by analogy.
