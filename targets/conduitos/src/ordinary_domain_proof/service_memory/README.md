# Held protected-service baseline (#6183)

The existing supported `cargo xtask make conduitos ordinary-domain-proof`
entrance now requires clean committed source and retains
`service-baseline-proof.json` beside its existing protection receipts. This is
an architecture proof appliance, not the normal product or a completed #6183
benchmark. No recorded measurements accompany this implementation yet. See
[evidence/manifest.json](evidence/manifest.json) for both retained compilation
failures and their exact sources. The first attempt stopped in existing base
Crèche code. After merging its reviewed repair, real compilation caught this
recorder's incorrect assumption that `Serial` implements `fmt::Write`. The
publication correction uses the existing `FixedText`/`early_write` mechanism;
that corrected source has **not** been compiled or executed. The mock API
harness missed this defect and is not freestanding compilation proof.

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
