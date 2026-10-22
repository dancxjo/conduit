# First protected-service baseline (#6183)

The supported `cargo xtask make conduitos ordinary-domain-proof` entrance
requires clean committed source and retains `service-baseline-proof.json` beside
its protection receipts. Clean `a04beed4a44405fe3a42369d67afd8d9183c4cf5`
completed the real x86_64 appliance at q35/max/one vCPU/256 MiB and published all
20 raw-TSC observations. This is an architecture appliance and an initial matrix;
full #6183 acceptance remains open. See the source/image-bound
[packet](../../../../../docs/audits/evidence/service-entry-baseline-20261022/README.md).

The later [setup-clearing extension](../../../../../docs/audits/evidence/service-setup-attribution-20261022/README.md)
adds 14 exact-instance observations around the actual service pool's existing
clearing call. Clean `ac82545031d10044a9341f01066289e759cb0e13` passed the same
full appliance: clearing occupied 93.5–96.7% of the observed setup interval.
These uncalibrated ratios include counter overhead and leave entry/trap/CR3
attribution, optimization and normal-product comparison open.

Historical `a77945244721f7ebf4a25ea69bacef789f76610f` compiled and linked,
but its 128 MiB profile refused `runtime-arena-unavailable` before protected
entry. Later 256 MiB admission failures and the exact text-binding diagnostic
are retained in the packet. The successful appliance changes no domain, heap or
work grant and establishes no 128 MiB minimum. Its measured source includes the
text/Morse admission repair and pending #6231 fault repair; it is not an
accepted-development or stable-release claim.

Both original compilation failures remain in
[evidence/manifest.json](evidence/manifest.json): the existing Crèche failure and
the recorder's invalid assumption that `Serial` implements `fmt::Write`. The
recorder uses production `FixedText`/`early_write`; two actual owning formatter
tests and two complete-line reader tests pass. The mock API harness is not
freestanding compilation proof.

The fixed recorder admits 20 observations: three trials each of terminal no-op,
eight-byte uppercase and 256-byte uppercase on fresh and reused service domains,
plus one fresh bounded-loop preemption and one fresh privileged-instruction
fault. The 256-byte ceiling is **only the existing TextFrame fixture capacity**;
it is not the maximum application transport or shared transfer-window grant.
Reused trial zero is that instance's first entry; trials one and two retain the
same address space. Fresh trials install distinct increasing instance identities.
Neither mode establishes a cold host, cold cache or calibrated physical latency.

Each sample retains raw LFENCE/RDTSC/LFENCE ticks for installation, request frame
clearing/staging, the whole `ServiceDomain::enter`, response status/length checking
and copying, and explicit quarantine. The request includes the existing frame's
input/output clearing. Response copy bytes are independently checked against the
uppercase result. Entry includes budget/context setup, guest execution, interrupts
and automatic quarantine on a fault. The fault sample's later quarantine field
measures only the idempotent follow-up call. Do not subtract it from entry time.
Reuse setup is charged to trial zero and explicit teardown to trial two.

Three adjacent counter-pair intervals are retained as overhead observations;
they are not subtracted and do not certify a noise floor. All observations use
fixed stack storage and publish serial only after the complete matrix. The
reader rejects absent, duplicated, truncated or reordered matrices, changed
calibration claims, reused fresh instances and missing loop preemption. The
receipt binds clean source, the actual appliance build ELF, ISO and transcript
hashes, QEMU version/profile and guest CPUID signature/features. Existing hostile
memory, sibling/Root, restart, FPU/register and timer coexistence proofs remain
required by this same entrance.

Remaining full issue work:

- Actual larger shared-window and staged IPC payload sweeps, including independently
  measured copy and entry boundaries at those admitted maxima.
- Further trials/noise controls as warranted; raw samples, no selective retries.
- Actual assembly boundaries and instrumentation-overhead controls before claiming
  independent trap, CR3, TLB or register-save attribution.
- Select an optimization only from a demonstrated cost. PCID has no implementation
  or approval through this baseline; feature detection, lifetime/invalidation and
  negative revocation/isolation proofs would be prerequisites.
- Source-pinned before/after benchmark and protection equivalence, plus normal
  responsive journey wall-time comparison through `native-latency-proof`.

Local preparation checks are distinct from actual freestanding compilation and
QEMU evidence. A normal 24-step journey alone does not supply this matrix.
