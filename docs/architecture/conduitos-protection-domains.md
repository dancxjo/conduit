# ConduitOS protection domains

Status: one bounded freestanding-emulator enforcement profile, owned by
[#3078](https://github.com/dancxjo/conduit/issues/3078). This is not a claim
about every ConduitOS architecture, hostile kernel providers, or DMA isolation.

The hostile-entry proofs are authorized defensive development of ConduitOS.
They execute repository-owned test implementations in local QEMU guests to
find and repair the new system's isolation holes. Probe addresses refer to
that guest's own Root, private domains, and emulated devices.

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
compiled image containing Unicode uppercase and the same portable Human
keymap implementation used by the Host into a CPL3 address space. The image
has its own Cargo manifest and lockfile and two immutable ELF load segments.
Its allocator refuses allocation; these implementations use bounded buffers
and retained keymap state in the first page of the private stack allocation. It uses
one entry and one terminal gate for the entire bounded input rather than a
privileged transition per character. Code is read-only/executable; constants,
input/output and stack have separate permissions; inherited Root mappings are
supervisor-only. Floating-point access traps instead of exposing Root register
state. A hard timer returns a hostile loop as a work-exhaustion fault. A fault
terminates the region; it does not resume the hostile instruction.

The ordinary five-placement text-and-timer Observatory proof also requires
`region/text` to retain its protected profile, isolation and budget preemption,
while `region/timer` retains its cooperative realization. Its completed domain
cost must match the actual sealed Plan/Play and fit the admitted memory
reservation; the original 4 KiB pure-placement check cannot substitute for the
backend and Root metadata reservation. `cargo xtask make conduitos prove
--arch x86-64` uses a private short QMP endpoint so the supported command also
works from long checkout paths.

Every ordinary product proof now checks its sealed five-placement text-and-timer
Plan against the current exact checked Source, using one shared conformance
projection. Placement references normalize to authored Gear identities; the
projection retains configuration, typed ports and semantic contracts, Cord
contracts and capacities, region membership, startup dependencies, cancellation,
completion, terminal and sign requirements. Machine identities, selected Backs,
provider bindings and physical memory differ independently. Startup must remain
a complete dependency-respecting permutation; equivalent independent ties may
change. Additional State, Fore, activation, fusion, pool, resource-Cord or Line
semantics refuse this bounded specimen check. Each receipt records the semantic
shape digest and its actual Plan identity; a shape check alone establishes no
processor confinement or accepted release.

The supplemental `cargo xtask make conduitos ordinary-domain-proof` lane runs
checked ordinary text Source through the production kernel and then checks 18
hostile entries: Root memory, capability memory, sibling memory, Root entry,
MMIO, ports, interrupt disabling, infinite loops with a clear or set direction
flag, floating-point access,
alternate syscall entries, division by zero, breakpoint, single-step, timestamp
access, code writes, and data execution. This lane does not establish that every
current product path uses protection domains. It also checks that Compose
state survives separate entries and that its Unicode output reaches the
protected uppercase implementation.

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
peak, six privilege transitions, and 118,784 backend bytes. Setup image copies and setup/teardown TSC ticks
are reported separately. Root metadata is charged to region admission; teardown
zeros the backend allocation. These are emulator measurements, with a shared
page and no ring slots. These counts describe the three-placement text run;
they do not describe the graphical Body run.

The native keyboard partition now prepares an exact provider scope and binds
its domain to the aggregate Body Plan and actual Body Play at activation. It
keeps the connected four-placement keyboard chain intact. Keymap and uppercase
execute in the domain; Root delivers canonical keyboard events and authorizes
serial presentation before retaining the operator-facing result. Cancellation,
input-provider loss, and execution failure revoke the domain. Other native
Plot implementations remain cooperative. The supported graphical `journey-proof` passed in the local x86_64 guest:
ordinary USB keyboard input selected Keyboard canvas and produced `A` from `a`
without replacing the Body Plan or Play. The captured display showed `A`.
Stop revoked the domain and zeroed its allocation. Its exact Body cost record
reported five entries and gates, one Base gate, 10 runtime copy bytes, ten
CR3/TLB switches and privilege transitions, a five-byte shared-window peak,
118,784 backend bytes and 6,324 per-domain Root metadata bytes. This includes
keymap initialization, press/release handling, uppercase and presentation;
it is not the total memory cost of the complete Body or its trusted kernel.
Keymap and uppercase now execute together in one entry. Root forwards the
precomputed result only after the existing kernel presents the matching
intermediate value on the original Cord; it performs no second uppercase
entry or transformation. The bounded result staging is charged to admission
and cleared on revocation. The focused chain fixture also expands `ß` to `SS`
in one entry. The first graphical checkpoint used six entries, twelve
privilege transitions, 11 copy bytes and a four-byte shared-window peak;
batching removes one entry and copy while retaining the same Source flow.
Memory Lantern's connected keyboard/keymap/edit/presentation region now has a
separate Body-bound domain admission. Its keymap and bounded retained editor
share one entry and private storage. The editor uses the same allocation-free
semantic owner as the hosted implementation; Root forwards its result through
the existing typed Cord and capability-gated presentation path. Preparation
reserves the domain and Root staging before Play. Cancellation retires both
text domains and clears pending results. The journey collector exercises
retention, deletion to an empty value, sibling presentation independence and
exact per-partition retirement costs. The canonical graphical proof passed locally: the editor recorded 21 entries,
five Base gates and 58 runtime copy bytes; each domain revoked on Stop and
zeroed its 118,784-byte allocation. Its per-domain Root metadata was 6,324 bytes.
The preceding keyboard measurements remain scoped to that separate partition.
A separate native diagnostic verifies that a one-byte editor preserves state
across capacity refusal and deletion. The mapped input still crosses its original
Cord; capacity refusal completes the Edit Host Call. The diagnostic is distinct
from the ordinary Body journey.
The native Tour Morse region now uses the same protected realization. Its
original Source fans the literal input independently to uppercase and Morse;
uppercase output is not the Morse input. Both transformations execute in one
pure domain entry using the canonical allocation-free semantic owners. Root
stages their bounded results and forwards them only through the original typed
Cords when the production kernel requests the matching branch. Either branch
may be requested first. A changed input cannot consume the other branch's
completion, and a Morse refusal does not alter the uppercase result.

Preparation checks the selected artifact and kind contract against the current
Host offer for all five placements. Text and indicator presentation receive
separate opaque handles, exact operation scopes and one-effect limits. Their
copy windows are bounded to 256 and 645 bytes respectively. Text's admitted
completion envelope retains its portable 256-byte limit; indicator completion
has no payload. A handle issued for one presentation cannot authorize the
other. Completion, cancellation and terminal gate faults revoke the domain.

All five supported ordinary-domain emulator lanes exercised the production
Tour kernel and runner, observed `SOS` and the canonical Morse pattern through
the serial provider, and verified completion revocation and full backend
zeroization. They retain identical Source, checked-plot and expanded-plot
identities; each architecture retains its own exact Plan and Play.
Every nonfixture Morse cost record reports five entries and scheduler returns,
two Base gates, 129 runtime copy bytes, a 45-byte shared-window peak and ten
address-space switches/TLB flushes. Per-target memory and setup costs are:

| Architecture | Backend reserved and zeroed bytes | Root metadata bytes | Setup image copy bytes | Observed privilege transitions |
|---|---:|---:|---:|---:|
| x86_64 | 118,784 | 66,272 | 23,005 | 12 |
| IA-32 | 131,072 | 65,616 | 23,854 | 12 |
| AArch64 | 126,976 | 66,288 | 22,080 | 12 |
| RISC-V64 | 118,784 | 66,272 | 17,946 | 10 |
| LoongArch64 | 126,976 | 66,280 | 23,576 | 10 |

The first three observations include one user interrupt; the last two include
none. Interrupt entries and their additional privilege transitions are
reported separately, so transition counts can vary across runs. These are
per-region costs, not the total trusted kernel or Body memory.
Independent fixtures on every lane verify text/indicator handle
cross-use refusal without an effect, replay refusal, either branch order,
changed-input refusal and independent Unicode transformation outcomes.
This is freestanding emulator evidence through a serial diagnostic provider;
it does not establish a physical indicator or a graphical Morse interaction.

Broader ordinary implementation coverage remains unfinished.
This is development emulator evidence, not accepted-release evidence.

The x86 budget now uses a separate Root-owned RTC periodic interrupt route.
Admission requires an unused RTC interrupt channel and verifies interrupt
delivery in Root with finite polling before entering hostile code. Three
1024-Hz ticks bound an entry; Root restores RTC configuration, PIC masks and
LINT routing afterward. Root owns the CMOS index port and keeps NMI enabled;
it never attempts to recover the mask by reading the write-only index port
([QEMU RTC implementation](https://github.com/qemu/qemu/blob/master/hw/rtc/mc146818rtc.c)). This does not reprogram the Source LAPIC/PIT timer.
The Source timer vector queues its original fact and resumes user execution;
only the budget vector can preempt the domain. Cost records include user
interrupt entries, Source timer interrupts and actual privilege transitions.

A separate native fixture arms the real Root Timer Base, runs a hostile loop,
and consumes exactly one matching wake after independent budget preemption.
It observed the Source interrupt in user mode on both the local x2APIC and
legacy PIC/PIT QEMU profiles. The legacy timer now clears an old IRQ0 while
the PIT counter is unloaded, before starting the new one-shot. This fixture
establishes timer coexistence at the mechanism boundary; it is not evidence of
a complete normal graphical Body run.

| Target | Ordinary protection evidence | Remaining boundary |
|---|---|---|
| x86_64 | Checked text Source, retained keymap and hostile-entry proofs; ordinary graphical Keyboard canvas uses actual Body-bound CPL3 execution and a gated serial effect | Broader ordinary implementation coverage, complete conformance and release acceptance remain unfinished |
| IA-32 | The normal legacy BIOS product's checked text region runs in CPL3 through the shared production adapter; independent emulator checks cover memory/privilege denials, capability/lifecycle refusals, loop preemption, floating-state restoration and Source timer coexistence | Broader ordinary implementation coverage, complete cost accounting and release acceptance remain unfinished |
| AArch64 | Supported emulator proof runs the normal text region at EL0, with a gated serial effect, completion revocation, nineteen independent boundary checks, floating-state restoration and Source timer coexistence | Broader implementation coverage, complete cost accounting and release acceptance remain unfinished |
| RISC-V64 | Supported emulator proof runs the normal text region in U-mode, with a gated serial effect, completion revocation, twenty-two boundary checks, floating-state restoration and Source timer coexistence | Broader implementation coverage, complete cost accounting and release acceptance remain unfinished |
| LoongArch64 | Ordinary text region at PLV3; normal product and independent boundary proofs passed on a locally corrected diagnostic emulator | Stock QEMU 10.2.1 refuses the unsupported counter control; no stock-emulator, hardware or release parity claim |
| ARMv6 | An explicit protected text request refuses before planning or cooperative kernel construction; the native A3 emulator lane verifies that disposition | No protected backend; its separately labeled cooperative A3 diagnostic establishes no confinement |

`ordinary_plan::prepare_protected` requires a protected text realization and
returns `protected-execution-unsupported` on builds without that backend.
It never falls back to cooperative preparation. Actual machine checks remain
mandatory on supported builds. This also gives additional cooperative profiles
an explicit refusal path instead of letting a protection request disappear.

`cargo xtask make conduitos run --arch armv6 --board rpi-b-plus-v1.2` verified
that refusal in the ARMv6 guest before its separate reviewed cooperative A3
diagnostic was prepared. The retained UART transcript and receipt distinguish
the refusal from the diagnostic's text/timer completion; the validator rejects
confinement claims and domain-cost records on that lane. The appliance now
reserves the current boot contract's 8 MiB arena. This is emulator evidence,
with no ARM user-mode, MMU-isolation or physical-board claim.

IA-32 owns a flat GDT and a bounded TSS with an out-of-range I/O bitmap.
Its fixed PAE tables preserve supervisor-only Root identity mappings and place
the immutable domain image at a separate 1 GiB virtual range. The low Root
image overlaps the 4 MiB range used on x86_64, so reusing that virtual range
would not preserve the running Root image. The domain's writable frame and
private stack are non-executable, with unmapped gaps between them. This backend
requires PAE, NX, SSE2 and genuine hardware entropy; it refuses unavailable
mechanisms. The supported IA-32 product emulator uses the `max` CPU profile.

The IA-32 image uses SSE instructions. Its transition saves Root floating
state, installs separate domain state and restores Root state on return;
interrupt handlers also protect the interrupted floating state. The normal
BIOS specimen completed its original two-region Source: text/upper ran in
CPL3, serial presentation crossed the admitted capability gate, and the timer
region retained its separate wake. The text domain was revoked and zeroed on
Play completion. The receipt measured three entries/gates, one Base gate, six
privilege transitions and CR3 reloads, 64 shared-window copy bytes, a 32-byte
window peak, 131,072 backend bytes and 21,878 Root metadata bytes. Root metadata
includes this composition's trusted production-kernel storage and bindings.
The copy and TLB counters currently describe shared-window transfers and CR3
reloads; complete accounting for floating-state transfers, paging-mode changes
and other scheduler copies remains unfinished.

`cargo xtask make conduitos ia32-ordinary-domain-proof` builds and boots the
normal live BIOS product twice before instrumenting it for independent IA-32
checks. Its version-2 receipt retains both normal Plan/Play and sealed
Observatory exports separately from the instrumented product. Normal boots
must preserve the image and checked Source/semantic Plan shape, refresh Host,
Boot, Plan and Play identities, and contain no diagnostic-negative entries.
Each normal UART/VGA record is retained before another boot replaces its
conventional path. Receipt-validation refusals stop and reap the guest.
The instrumented image checks seventeen memory/privilege/loop cases and the
shared capability/lifecycle checks.
The two loop cases require three budget interrupts before returning, including
one case with the user direction flag set. Root interrupt entry clears that
flag before calling Rust. The IRQ frame accounts for the assembler's two-byte
segment saves when locating the interrupted privilege level. Floating state is
compared immediately after restoration, before Rust can alter caller-saved
registers. A separate pending Source timer delivers exactly one wake while
the independent RTC budget preempts user execution; retiring Source IRQ0
preserves the RTC cascade's PIC mask. These are local emulator results, not
physical-machine or accepted-release evidence.

AArch64's development backend uses 4 KiB translation tables and a bounded EL1
exception stack. Each domain has immutable EL0 code, non-executable private
storage, and privileged inherited Root mappings. A separate physical timer
returns control after three budget interrupts; the existing virtual timer
retains Source wake ownership. The reviewed emulator CPU is `neoverse-n2`,
whose architectural RNDR provider supplies opaque capability material.
Unsupported translation regimes or absent entropy refuse admission.

`cargo xtask make conduitos aarch64-ordinary-domain-proof` completed the normal
AArch64 product's original text-and-timer Source
with three EL0 entries, one serial Base gate, six translation/TLB switches,
64 shared-window copy bytes, a 32-byte window peak, 126,976 backend bytes and
22,390 Root metadata bytes. Completion revoked the domain and zeroed its
backend storage. An independently instrumented image exercised nineteen
memory, privilege, alternate-gate and loop cases, the shared capability and
lifecycle checks, and exactly one Source wake during budget preemption.
The image also changes all `q0`–`q31` registers and FPCR/FPSR before both a
normal gate and an infinite loop. Assembly compares the restored Root state
with its entry snapshot before any Rust executes, including the timer IRQ
handler; an incorrect restoration makes the proof refuse.
The supported entrance boots the normal image twice, then independently
instruments and boots the boundary-check image twice. Both product receipts
require the exact completed domain's Plan/Play, capability gate and zeroed
storage; an ordinary ready Sign alone cannot satisfy them. These local emulator
results do not establish physical hardware or release acceptance.
The normal AArch64 product prepares a bounded Observatory export before sealing
its allocator, then emits the sealed Plan and completed Play after execution.
Both boots must retain one complete export whose Source, checked/expanded Plot,
Plan, Play, Host/Boot, offer generation and image/build provenance match the
product receipt. Missing, truncated, duplicate or stale exports cannot satisfy
product proof. The remaining full cost-accounting
gaps above also apply to this backend.

RISC-V64's development backend uses Sv39 with immutable U-mode code and
non-executable private frame and stack pages. Root validates every inherited
high-half page-table leaf as supervisor-only, with a finite descriptor budget.
The user trap swaps to a Root-owned stack without dereferencing the user stack,
denies supervisor CSR access and SBI requests, and restores Root's trap and
translation state on return. Its SBI timer multiplexes independent Source and
budget deadlines: one physical interrupt may satisfy both, while the Source
wake keeps its original owner. Three budget interrupts return a looping domain.
Admission requires the reviewed translation regime and SHA-256-conditioned
Zkr entropy; missing mechanisms refuse rather than supply weaker capability
material. The reviewed emulator CPU is `rv64,zkr=true,sv57=off,sv48=off`.
The supported proof-tool preparation entrance is
`cargo xtask make conduitos prepare-riscv64-domain-emulator`. It builds pinned
QEMU 10.2.1 and verifies pinned OpenSBI 1.8.1 firmware before selecting them.
The CI QEMU 8.2/OpenSBI 1.3 pair refuses the required supervisor entropy access;
upgrading either component alone still refuses. The reviewed pair completed
the ordinary product boot locally with the same older U-Boot artifact.
Tool preparation is separate from execution proof and records both content
identities; it does not weaken entropy, page or capability admission.


`cargo xtask make conduitos riscv64-ordinary-domain-proof` completed two normal
product boots and two independently instrumented boots. The original
text-and-timer Source completed with three U-mode entries, one serial Base gate,
six translation/TLB switches, 64 shared-window copy bytes, a 32-byte window peak,
118,784 backend bytes and 22,374 Root metadata bytes. Completion revoked the
domain and zeroed its backend storage. Twenty-two memory, privilege, firmware,
alternate-gate and loop cases passed, alongside shared capability/lifecycle
checks and exactly one Source timer wake during budget preemption. Independent
code changes all `f0`–`f31` registers and FCSR before a gate and a loop; assembly
compares restored Root state before Rust resumes or handles an interrupt.

The normal RISC-V product now uses the bootloader-normalized memory map,
executable extent and HHDM rather than a fabricated boot record. Its Make admits
the normalizer's 8 MiB preparation arena and a finite 1 MiB preparation stack.
The normal product seals allocation before Play; the separately instrumented
image permits subsequent diagnostic Play preparation. The product receipt
requires the exact completed domain, and its Patchbay projection retains the
actual UEFI handoff provenance. These are local emulator results; the full
cost-accounting, physical-hardware and release limitations above still apply.

LoongArch64's preparatory product path now uses normalized boot memory and
executable bounds, with the normalizer's admitted 8 MiB arena. Its requested
Limine base revision 6 defines the privileged entry, page-table roots and
4 KiB paging regime; Root enables the scalar floating bank required by its
Rust target ABI. The retained provenance reports the same requested revision.
The normal product completed two independent emulator boots, retaining its
text-and-timer result and native Patchbay projection. This establishes the
boot facts needed to construct domains.

The reviewed LoongArch virtual profile now supplies a fresh, finite 4 KiB
cryptographic input from Linux `getrandom` through a private emulator firmware
file. Root reads the exact read-only firmware descriptor without DMA, retains
the input only in privileged storage, clears consumed bytes and refuses further
requests when the pool is exhausted. Acquisition has no timing/counter fallback.
The host removes its private input file when the guest ends. Two independent
direct-boot diagnostics of the actual provider checked exact fills, interrupt
state restoration, finite capacity and consumed-storage erasure; a missing-input
boot checked refusal and cleared output. The normal product also completed two
boots with the host input integration. This is local virtual-platform provider
evidence, not a physical LoongArch RNG or a protected-execution claim.

The ordinary LoongArch text region now uses the shared production adapter at
PLV3. Root creates fixed four-level 4 KiB address spaces, boundedly verifies
inherited high-half leaves for PLV0-only access, removes inherited low mappings,
and admits only immutable code/constants, the shared frame and a private stack.
Root owns a stackless, bounded TLB refill walk which stops at missing directories
and retains all page protections, including NX. The private exception stack
does not rely on the application stack. The syscall gate checks the exact
instruction from Root's immutable admitted code copy at the bounded saved PC.
All scalar floating registers, FCSR and condition flags are restored before
Root Rust or its IRQ handler executes. The independent countdown budget returns
unresponsive entries after three interrupts while retaining a Source timer wake.

`cargo xtask make conduitos loongarch64-ordinary-domain-proof` passed two normal
product boots and two instrumented boots. The normal original text/timer Source
completed with three PLV3 entries, one exact UART capability operation and
completion revocation; its receipt records 126,976 backend bytes, 22,382 Root
metadata bytes and `rdtime` timing. Twenty-three independent memory, privilege,
gate, loop and invalid-stack cases passed, as did the shared capability/lifecycle
refusals, floating-state comparison and Source timer coexistence.

These results use a locally built QEMU 10.2.1 with the two reviewed
[diagnostic corrections](../../targets/conduitos/proof/tools/loongarch64/README.md).
`cargo xtask make conduitos prepare-loongarch64-domain-emulator` now prepares
that tool from a digest-checked source archive and both repository patches.
It retains the build recipe, observed tool versions and executable digest;
warm selection verifies the receipt before using the executable. The LoongArch
CI lane acquires the required development packages and uses the same entrance.
Stock QEMU marks MISC read-only; Root returns the explicit
`protected-execution-unsupported` refusal before application entry. A second
emulator correction prevents recursive faults while recording inaccessible
instructions. The receipt records the actual emulator version and executable
digest. This diagnostic proof does not establish stock-emulator support,
physical execution or accepted-release parity.

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
