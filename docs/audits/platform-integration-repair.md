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
