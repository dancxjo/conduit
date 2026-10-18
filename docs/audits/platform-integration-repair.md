# Repair the native and browser integration boundaries

Integration run [39067896773](https://github.com/dancxjo/conduit/actions/runs/39067896773)
exposed three deterministic build/package regressions and one crates.io
connection failure. This repair is a prerequisite to admitting the prepared
expression work in #5396; it changes neither numeric semantics nor acceptance
requirements. The RP2040 dependency-download failure is infrastructure evidence,
not a firmware test failure or successful target proof.

## Pinned AVR compiler

The AVR target pins `nightly-2024-07-22`. Newly used `Option::is_none_or`
does not compile there. Equivalent matches now preserve the endpoint-prefix
and fixed/allocated Sign byte-pressure decisions without requiring a newer
library API or an unstable feature. Overflow still produces pressure.

The unmodified firmware compilation reproduced the four reported API errors.
The repaired firmware passes `cargo check --locked --release` on its pinned
compiler and actual custom AVR target. A separate no_std, allocation-enabled
kernel check also passes on that compiler, covering the allocated Sign path.
The local machine lacks `avr-gcc`, so this is compiler evidence rather than
firmware linking, execution or physical hardware evidence.

The kernel all-features regression suite passes 240 tests with five existing
diagnostic tests ignored, including the ordered-prefix oracle and Sign bounds.
Strict kernel library Clippy passes with warnings denied.

## Std scheduler preparation stack

Candidate [39069319654](https://github.com/dancxjo/conduit/actions/runs/39069319654)
also exposed a default-stack overflow in the Patchbay HTTP route regression.
The unchanged test reproduces locally. A debugger locates the overflow in
kernel topology validation reached through several wide, by-value scheduler
construction frames in std preparation.

Std now calls the kernel's existing boxed preparation entrance directly instead
of constructing a scheduler by value and boxing it afterward. It retains the
same fixed scheduler, topology/Host-binding validation and pre-Play allocation
boundary. The complete thirteen-test HTTP server suite passes with its original
stack settings and assertions, including the previously aborting route test.

## Fixed protected image window

The x86_64 independently linked image exceeded the existing 128 KiB admitted
code/constant window. Its immutable load segments ended at `0x4200e8`, requiring
135168 page-mapped bytes from `0x400000`. The loader correctly refused it.

The x86_64 domain now uses whole-image LTO, as LoongArch64 already does, while
retaining the same source, linker mapping, immutable permissions and 128 KiB
limit. The measured normal image ends at `0x41d020`, requiring 122880 mapped
bytes. This records the linked layout, not processor-enforced execution proof;
the ordinary domain emulator proof and target CI retain their own obligations.
Other architectures retain their previous optimization choice.

## Browser package resource admission

The current durable storage module is 18918 bytes, exceeding Workspace's stale
16384-byte module declaration. Its Face module is 44141 bytes, exceeding its
40960-byte declaration. Workspace now declares finite 24576-byte and 49152-byte
package resource envelopes. This does not change IndexedDB data quotas or remove
the package producer/consumer bound checks. The package digest commits to the
new declaration and actual asset hash.

The focused regressions check every shipped Workspace module's byte admission
and feed the actual storage source module and consumer declaration
through the real package producer, checks its retained hash/bound, and confirms
that a one-byte-too-small declaration still refuses. It is part of
`cargo xtask check browser`. This is package admission proof; complete staging,
browser execution and release publication remain separate CI evidence.

## Process allocation measurement exclusion

Candidate runtime-performance conformance failed with `nested allocation
measurement` in the fan-out pressure cancellation test. That test omitted the
process-wide measurement mutex used by the rest of the hosted pipeline corpus;
its Host preparation can overlap another test's active counter window. It now
uses the same test-only exclusion guard. The production allocator and cancellation
assertions are unchanged; this does not serialize product execution or convert
a missing measurement into zero allocations.

## Browser exclusive-choice projection

The replacement candidate passed every native lane, all std/browser/workbench
unit shards and its runtime-performance report. Foundation exposed the separate
Patchbay invalid-control fixture correction owned by #5958. The browser suite
passed 254 cases but Thermostat could not locate its Heat radio control.

The retained screenshot shows generic fallback cards for known choice groups.
The actual browser projection includes all properties in its generic facts list,
including selection flags and choice metadata already represented by typed fields.
The renderer incorrectly required that complete facts list to be empty. Handwritten
renderer fixtures omitted the duplicated properties and missed the mismatch.

The projection now reports the count of unmodeled properties separately, retaining
the complete facts list. The radio profile requires that count to be exactly zero,
plus its existing exact role, multiplicity, selection, action and input checks.
Unknown metadata and duplicate/contradictory multiplicity retain the generic
fallback. Missing count evidence also retains that fallback. A regression uses
the real Thermostat contribution, including adversarial extra properties.
This changes presentation eligibility, not action authority or thermostat state.

Clean code source `a519da5ff6a809930c2ca463aeca3804d88b042f` seals a
18,964,181-byte browser runtime inside its unchanged 20 MiB admission limit.
The supported static application packager stages that exact runtime; the unchanged
Thermostat test passes with pinned Chromium, one worker and zero retries. The
existing custody collector accepts its source/runtime, selected logical Back,
accessibility, pressure, refusal and recovery evidence. An initial successful run
omitted the source-pin environment; its log is preserved separately and its receipt
was not relabelled. The subsequent custody capture contains the actual source pin.

A follow-up closed-profile guard retains nested choice contracts in the generic
view instead of hiding their metadata inside a radio option. Its new regression
fails before the guard and all 96 renderer fixtures pass after it. This guard
changes unsupported nested-contract handling; the actual Thermostat capture
retains its original source identity. Native projection regression, strict browser
library Clippy and formatting also pass. See the
[receipt and scope record](evidence/browser-choice-projection-20261017.json) and
[raw evidence archive](evidence/browser-choice-projection-20261017-raw.tar.gz).
Full combined Candidate, integration and exact stable publication remain required.

## Refreshed accepted foundation

Merged accepted dev `45a6384eafe3bf8c2d29b843421adbb2766e8396` into this
repair. All 44 Patchbay application tests pass on combined source
`b98cead003a3bd5eee96561f6c649d66a61d5fa8`, including the previously failing
resident-canvas case. The boxed std preparation and allocation-measurement mutex
corrections are now in accepted dev and are no longer unique changes in this PR.
The source-specific native and browser receipts above retain their original pins;
the refreshed Candidate must establish acceptance of the final combined tree.

## Complete pinned AVR firmware link

After provisioning the pinned project-local tools through `cargo xtask make avr
check`, clean repair source `2e4b99d4d1ec71896be9be2756b3d824c4494145` passes
`cargo xtask make avr release --output target/repair-avr-release`. This supersedes
the earlier local linking limitation without changing any firmware resource
limit. The linked application uses 27,316 of 27,648 flash bytes and 1,387 of
1,460 static SRAM bytes, retaining the existing 1,100-byte stack reserve.
The supported release adapter seals the actual Intel HEX and its source/hash.

The [receipt](evidence/avr-platform-repair-20261017.json) and
[raw archive](evidence/avr-platform-repair-20261017-raw.tar.gz) retain setup and
build logs, complete pinned-core verification, compiler identity, build receipt,
release manifest and exact HEX. This is machine-only compile/link/admission
proof, not flashing, hardware execution or a measured stack peak. Full combined
CI and stable publication remain separate gates.
