# ConduitOS protection domains

Status: architecture-neutral ordinary execution-region protection with five
reviewed native emulator realizations, owned by
[#5113](https://github.com/dancxjo/conduit/issues/5113). ARMv6 explicitly refuses
protected execution. The original x86_64 adversarial profile
[#3078](https://github.com/dancxjo/conduit/issues/3078) remains a separate proof.
Driver, DMA and physical-machine isolation are not established by these lanes.
The [microkernel privilege boundary](conduitos-microkernel.md) tracks moving
production services out of Root; existing region proofs do not establish it.

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
authorized serial face through the capability gate. The kernel resumes
the bounded fixture after expected protection faults and terminates on any
unexpected vector or sequence.

Acceptance requires both the structured protection sign and the independently
observed `CONDUIT_SERIAL_PRESENT protected-domain-authorized-effect` line. The
sibling sentinel is independently checked by the kernel before success. A
protection fault is recorded as a refusal fact, never semantic completion.

## Ordinary protected execution (#5113)

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

The normal text-and-timer proof collectors check their sealed five-placement
Plan against the current exact checked Source, using one shared conformance
projection. The x86_64 collector runs through `cargo xtask make conduitos prove`;
its graphical product journey proves separate Body-bound domains. Placement references normalize to authored Gear identities; the
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

The text region requests one serial face through an opaque authenticated
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
serial face before retaining the operator-facing result. Cancellation,
input-provider loss, and execution failure revoke the domain. Other native
Plot implementations remain cooperative. The supported graphical `journey-proof` passed in the local x86_64 guest:
ordinary USB keyboard input selected Keyboard canvas and produced `A` from `a`
without replacing the Body Plan or Play. The captured display showed `A`.
Stop revoked the domain and zeroed its allocation. Its exact Body cost record
reported five entries and gates, one Base gate, 10 runtime copy bytes, ten
CR3/TLB switches and privilege transitions, a five-byte shared-window peak,
118,784 backend bytes and 6,324 per-domain Root metadata bytes. This includes
keymap initialization, press/release handling, uppercase and face;
it is not the total memory cost of the complete Body or its trusted kernel.
Keymap and uppercase now execute together in one entry. Root forwards the
precomputed result only after the existing kernel presents the matching
intermediate value on the original Cord; it performs no second uppercase
entry or transformation. The bounded result staging is charged to admission
and cleared on revocation. The focused chain fixture also expands `ß` to `SS`
in one entry. The first graphical checkpoint used six entries, twelve
privilege transitions, 11 copy bytes and a four-byte shared-window peak;
batching removes one entry and copy while retaining the same Source flow.
Memory Lantern's connected keyboard/keymap/edit/face region now has a
separate Body-bound domain admission. Its keymap and bounded retained editor
share one entry and private storage. The editor uses the same allocation-free
semantic owner as the hosted implementation; Root forwards its result through
the existing typed Cord and capability-gated face path. Preparation
reserves the domain and Root staging before Play. Cancellation retires both
text domains and clears pending results. The journey collector exercises
retention, deletion to an empty value, sibling face independence and
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
Host offer for all five placements. Text and indicator face receive
separate opaque handles, exact operation scopes and one-effect limits. Their
copy windows are bounded to 256 and 645 bytes respectively. Text's admitted
completion envelope retains its portable 256-byte limit; indicator completion
has no payload. A handle issued for one face cannot authorize the
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

### Private standing timer execution

The native count-over-time product now prepares a protected three-placement
region for Every, Count and Count Face. Root retains the verified
Plan/Play binding, numeric preparation data, capability table and provider
tokens. The separately compiled domain image owns the production fixed
scheduler, Count state, Cord movement and decimal rendering. Root enters at
actual effect/completion boundaries; an idle domain stays suspended while its
physical timer is pending. Other semantic families retain their separately
declared execution profiles.

Timer and Count Face receive separate opaque handles. Admission pins
Host/Boot, Plan/Play, implementation artifact and contract, Base/resource
identity and generation, operation, parameter/work limits and independent
in-flight/operation bounds. Every's selected provider identity seals both its
physical Timer and Clock; replacing either changes the current scope. A timer
lease remains in flight until the exact current provider wake matches the
entire pending interest. Count's semantic input is eight bytes; its physical
face window allows at most twenty decimal digits. Root validates that
window without computing or substituting the semantic count.

The reviewed finite product arms 120 milliseconds. Conversion rounds upward
using the platform frequency. IA-32 retains one logical timer across bounded
PIT counts; intermediate interrupts do not publish a semantic wake. AArch64
uses CNTFRQ, LoongArch64 uses the CPUCFG counter-frequency ratio, and the pinned
RISC-V QEMU virt profile uses its 10 MHz timebase. x86_64 waits against its
admitted calibrated TSC/HPET counter. An unavailable duration basis refuses
instead of substituting the diagnostic timer arm.

The shared image now admits a 128 KiB immutable code/constant window, a 32 KiB
non-executable stack and 16 KiB of private non-executable retained state.
The numeric graph wire fits within 256 bytes; mutable shared request storage
is a separate 4 KiB page. Guard gaps separate these windows. Architecture
translation/exception storage and Root metadata remain separately accounted
and admitted. The earlier text/Morse measurements in this document record the
former image profile; new receipts report the enlarged reservation and exact
zeroization rather than reusing those historical costs.

Stop cancels the physical arm before revoking capabilities and zeroing domain
storage. A retired owner refuses a second run before touching any provider.
The native proof exercises the ordinary Tour product entrance with actual
providers, separately from the explicitly synthetic private-runtime fixture.
Its timer receipt retains effect gates, copies, shared-window occupancy,
interrupts, setup/teardown, address-space switches and memory. Local proofs
passed on all five supported protected architectures, including the actual
120 ms product and terminal replay refusal. Each used eleven domain entries,
four Base gates, 313 runtime bytes copied, a 163-byte shared-window peak,
22 address-space switches/TLB flushes and eleven scheduler returns. The product
completed without budget preemption; independent hostile-loop fixtures still
prove bounded preemption. Teardown zeroed the entire reservation in every case.

| Target | Privilege transitions | User IRQ entries | Setup bytes copied | Reserved / zeroed bytes | Root metadata bytes |
|---|---:|---:|---:|---:|---:|
| x86_64 | 38 | 8 | 71,639 | 217,088 | 5,280 |
| IA-32 | 38 | 8 | 64,865 | 229,376 | 5,008 |
| AArch64 | 32 | 5 | 64,460 | 225,280 | 5,296 |
| RISC-V64 | 22 | 0 | 54,456 | 217,088 | 5,280 |
| LoongArch64 | 22 | 0 | 67,796 | 225,280 | 5,288 |

| Target | Setup ticks | Teardown ticks | Counter unit |
|---|---:|---:|---|
| x86_64 | 6,918,183 | 26,234,556 | TSC |
| IA-32 | 9,068,216 | 20,430,106 | TSC |
| AArch64 | 189,101 | 710,699 | CNTVCT |
| RISC-V64 | 43,730 | 195,497 | time |
| LoongArch64 | 291,825 | 1,678,133 | RDTIME |

These are retained emulator measurements from one candidate, with
architecture-specific counters; they are not cross-platform latency rankings.
The private image uses speed optimization and thin LTO, and initializes its
kernel directly in retained state. This removed IA-32 initialization overhead
that exhausted the existing watchdog; its three-interrupt budget was unchanged.
Release acceptance is recorded separately from these local emulator receipts.

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
CPL3, serial face crossed the admitted capability gate, and the timer
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
checks. Its version-3 receipt retains both normal Plan/Play and sealed
Observatory exports separately from the instrumented product. Normal boots
must preserve the image and checked Source/semantic Plan shape, refresh Host,
Boot, Plan and Play identities, and contain no diagnostic-negative entries.
Each normal UART/VGA record is retained before another boot replaces its
conventional path. Receipt-validation refusals stop and reap the guest.
The instrumented image checks seventeen memory/privilege/loop cases and the
shared capability/lifecycle checks using QEMU instruction time (`shift=0`,
`sleep=off`, RTC `clock=vm`), recorded in the receipt. Normal product boots keep
host time. Instruction time isolates the three-interrupt budget from host
scheduling; it provides no cycle-accurate performance evidence.
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

The ordinary text serial face now passes through its domain capability
gate, and the supplemental Sign reports `effect_capability_gates:true`.
The reviewed protected text, Keyboard, Memory Lantern, Morse and standing
timer paths do not implicitly protect other native workset implementations.
DMA and driver isolation remain false.
Issue #5113 stays open until its complete cross-architecture product and proof
criteria are earned; the local emulator run is not accepted-release evidence.

## Private compositor service memory prerequisite (#6181)

The development x86_64 backend also admits one independent service slot, separate
from the two ordinary-domain slots. Its immutable ELF window is 2 MiB; private
RW/NX grants provide a 4 KiB exchange, 64 KiB transfer window, 64 KiB stack,
256 KiB metadata, 8 MiB retained pixels and 8 MiB transaction scratch. Every gap
and both outer guards remain unmapped. Page tables and the supervisor trap stack
are charged in addition to user memory. Ordinary text/timer layouts are unchanged.

`ServiceDomain` reuses the existing CPL3 transition and RTC preemption mechanism.
Each call admits one hardware-budgeted entry; protocol operations must separately
bound their own work. Root can stage the fixed transfer window only between
entries. Faults quarantine the instance, clear the entire private slot and refuse
further entry, exchange or transfer access. A replacement receives a new boot-local
instance identity. This identity grants no Host/Boot capability or device effect.

`cargo xtask make conduitos ordinary-domain-proof --locked` includes the actual
service-memory backend fixture. It checks retained boundary reads/writes, Root
staging visibility, 26 hostile entries, Root/sibling survival, guard and W/X
refusal, port/MMIO refusal, loop preemption and restart. Zeroing observations are
boundary samples plus whole-slot charged clearing, not an empirical readback of
every private byte. The receipt binds the actual appliance ELF and serial hash,
records the explicit 128 MiB QEMU fixture profile, and reports
`production_compositor:false`, `driver_isolation:false`, `dma_isolation:false`.
The earlier 64 MiB guest refused its runtime arena before any protected entry;
this fixture profile does not define a graphical product minimum.

This is infrastructure for [#6177](https://github.com/dancxjo/conduit/issues/6177).
It does not move the production retained compositor, grant scanout authority,
establish unchanged graphical behavior or complete the microkernel boundary.

## Native graphical compositor service (#6177)

The x86_64 freestanding native graphical path now selects an independently
linked compositor service. The other native architectures keep their existing
renderer until each has its own service backend and evidence. Root retains
validated scanout and semantic Face/Mask/Show bindings. A temporary readable
raster lease supports the existing typography/paint realization; the service
alone owns committed pixel arenas, surface lifecycle, damage, z-order, visual
focus/cursor, composition and advisory route selection. No Root pixel-buffer
fallback is selected on service failure.

A display-local owner is prepared only after provider/resource checks and is
bound to the current Host, Boot, offer generation, framebuffer Base and admitted
scanout format. Presenter-local namespaces fence old handles/routes across
handoff. Root checks the versioned request/instance/sequence envelope and admits
the complete finite damage plan before accepting ordered output spans. It copies
every declared span before sending scanout acknowledgement; guest completion
alone cannot acknowledge a semantic Show. Borrowed acknowledgement evidence also
holds the shared owner against entry or handoff during semantic completion.
Actual faults or partial output invalidate evidence and terminate/quarantine the
current owner; there is no automatic retry, image replacement or semantic Show
reuse. Backend reinstall machinery does not yet establish independent production
restart acceptance.

`cargo +stable xtask make conduitos journey-proof --locked` exercises the normal
graphical path, including Crèche, Body lifecycle, protected applications,
Patchbay and Mask interactions under the normal 512 MiB QEMU profile. Nonempty
`CONDUIT_COMPOSITOR_SERVICE_FRAME` records expose cumulative entry, privilege
transition, address-space-switch and scheduler-return counters after successful
copies and acknowledgement. These are source/artifact-bound emulator evidence,
not hardware attestation or physical scanout receipts. Generated names and Host
identities can differ across captures; identical appearance must be assessed
against semantic inputs, not asserted from different PNG hashes alone.

The private-memory fixture above remains a separate mechanism proof. A passed
ordinary journey does not itself prove hostile production-proxy replies,
Root/sibling survival through that proxy, independent restart, other architecture
confinement, accepted release, driver/DMA isolation or the complete microkernel.
The evidence above predates the Crèche service work described below. Product
control, general Face/Mask producers, the portable execution runtime and
network/device services remain migration work under
[#6175](https://github.com/dancxjo/conduit/issues/6175).

## Crèche service implementation in progress

The x86_64 native Arrival path now routes draft ownership through a separate
Crèche executable. That executable uses the existing canonical `BirthDraft`,
`BirthActions`, widget and Face projection code. Root retains immutable guest
snapshots and its reviewed Plot catalog, checks returned selections against that
catalog, and continues to own Body admission and lifecycle effects. This does
not move the complete product controller or general Face/Mask machinery out of
Root. The native Crèche controller now also retains keyboard composition, focus,
name replacement and refusal handling inside that guest. Root forwards a
revision-bound three-byte portable key event and consumes a finite outcome plus
read-only projection; unchanged composition events do not advance the outer
FrontDoor revision. The same guest now constructs the native Crèche scene and
Host Face projection from explicit viewport dimensions and scoped Host/Boot/
revision values. Root decodes the existing canonical scene codec, checks the
request binding and viewport clips, and validates returned Face scope, revision
and content. No framebuffer address or PixelTarget reference crosses this
protocol. Host and other-architecture controller/presentation paths remain local.

Home launcher/command-bar state also runs in an independent Home-profile guest:
the canonical HomeModel, keyboard composition and refusal state are guest-owned.
Root consumes validated read-only snapshots and application views, checks current
installed Plot indices, and retains lifecycle effects. Explicit reopen resets a
live guest; terminal recovery releases the old owner before fresh installation.
Home layout and local Face content also execute in that guest. Root sends only
bounded viewport, identity, installed-Plot and lifecycle display facts; it checks
request correlation, canonical scene decoding, exact viewport clips and finite
Home-subject Face content before assembling the checked outer Face. Outer
Face/Mask/Show acknowledgement and physical display admission remain in Root.

The closed service pool has independent compositor, Draft, Home, Runtime and Application slots. Each image
is bound to its profile; instance and request identities fence the bounded wire
exchange. Draft messages use finite chunked blobs, with a 128 KiB aggregate
ceiling. A terminal transport failure quarantines and releases the owning slot.
The standalone guest compiles with separate immutable code/constants and a
private writable non-executable initialized-data page. Heap, complete stack
depth and the finite Draft entry budget have not yet been measured at runtime.
Earlier compositor journeys do not validate this later configuration.

The controller platform adapter now owns hardware input handles and exposes
typed events plus checked display effects to controller policy. This is a
preparatory source boundary within Root, not controller hardware isolation.
Expanded emulator proof, CI acceptance and physical/driver/DMA evidence are
deferred while local implementation proceeds in draft PR
[#6196](https://github.com/dancxjo/conduit/pull/6196).

## Native workset execution runtime in progress

The x86_64 native Body workset now constructs its existing canonical
`FixedScheduler`, numeric Backs, value store, Cords and sign log in a separate
Runtime executable. Root submits the checked numeric admission and services
bounded Host Call requests. Host and other-architecture worksets use the same
shared Back definitions and canonical scheduler locally. Tour/ordinary runtime
paths and canonical application Host Call adapters still execute in Root; this
is not a claim that the whole portable engine has left Root.

The Root broker independently checks admitted nodes/calls/byte envelopes,
pending completion identities, strictly increasing request IDs and at most one
new request per node between Root step commands. Single-use Root-issued value
loans bind effect inputs to exact completed bytes and reviewed downstream routes.
This authority ledger does not choose runnable work or execute another scheduler.
Initial empty Application/Edit flows have finite startup grants. Terminal errors
release the guest and fence queued application actions, held inputs and unpublished
views; a failed completion cannot publish a newly queued lifecycle action.

A targeted native compile passes. Inspection of the first linked guest found a
392,280-byte dispatch frame and a nested 141,176-byte scheduler construction
frame, exceeding the initial 256 KiB stack. The corrected closed Runtime profile
uses a 1 MiB stack and 1 MiB retained metadata; a compile-time assertion bounds
retained storage. These observations do not establish full peak stack depth,
execution timing, adversarial containment, physical execution or acceptance.
Expanded proof and CI validation remain deferred for the local implementation
workflow in draft #6196.

## Native retained application models in progress

The native workset's Tutorial, Tour and Patchbay retained models now execute in
an independent Application guest. Tutorial uses its existing biography-derived
projection; Tour uses the canonical application port; Patchbay uses the same
canonical port over Root-reviewed graph facts. Expanded planners are not sent
across this boundary. Host/other-architecture worksets use the same application
core locally. The existing expanded-Plot constructor retains its previous
validation behavior; the new projected-graph entry adds finite transport bounds.

Root holds immutable admission graphs, prior views, selection context and live
Mask topology. A returned effect proposal must match genuine admitted input and
that prior view: Tutorial's finite actions, Tour's current specimen/run context,
or Patchbay's reviewed subject and exact Body Plan/Mask mode. Empty inputs and
control/refresh/completion replies cannot mint actions. This gate does not run a
second mutable application controller. Termination fences pending actions and
unpublished views and releases the Application lease.

The Application image fits its closed RX/RO window and uses a separate 16-byte
initialized RW/NX metadata template. Native compilation passes; entry-frame and
segment inspection do not establish full stack/heap fit or execution acceptance.
The separate ProductJourney lifecycle Tutorial projection/action adapter remains
in Root, as do actual lifecycle/effect admission, general Face/Mask and the
remaining execution/network/device paths. Expanded runtime proof is deferred.

## Deliberate limits

Each reviewed emulator profile proves its own CPU page, privilege, handle and
admitted Base boundaries. Architecture-specific evidence above does not prove
an IOMMU, DMA containment, mutually isolated kernel drivers/providers, physical
hardware execution, SMP or an unsupported backend. Receipts report
`dma_isolation:false` and `driver_isolation:false`. Each architecture earns its
own mechanism-level evidence; none inherits another target's proof by analogy.
