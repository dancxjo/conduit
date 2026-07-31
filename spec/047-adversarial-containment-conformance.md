# Adversarial containment conformance current form

Status: C5 verification contract

This program tests valid-but-dangerously-composed operations across authority,
realms, artifacts, persistent budgets, recovery, federation, implementation
confinement, and hazardous-host state. It supplements parser fuzzing and local
contract fixtures; it does not prove arbitrary software or physical systems
safe.

## Trace contract

Each retained trace names its initial state, attacker capabilities, ordered
operations, expected rejection or commit point, final protected state, and
stable evidence. The hosted reference dispatcher independently executes every
named trace. A rejection snapshots authority, population, install/federation
state, persistent ledger state, host resources, and inhibit state before the
production validator call and requires byte-for-byte-equivalent protected
state afterward.

After every accepted or rejected step the harness checks:

- authority and delegation never widen beyond the independent ceiling;
- population, installation, federation, and persistent budget counters remain
  within their cumulative ceilings;
- lifecycle, epoch, run, generation, and realm changes do not replace the
  authoritative budget ledger;
- rejected consequential effects allocate no downstream host resource;
- an inhibited host retains a zero plan, epoch, command authority, and lease;
- trace and evidence storage remain bounded.

The corpus principally uses production functions from the containment, realm,
genesis, artifact, plan-graph, distributed-session, policy-budget, runtime
evidence, and inhibit contracts. Fake state supplies only deterministic host
storage and attacker scheduling; it does not replace the validator under test.

## Campaigns and replay

Ordinary CI executes all retained traces twice and compares the complete
result/evidence sequence, then runs 64 deterministic selections under seed
`1380991557`. The scheduled Security workflow runs 4,096 selections with the
same dispatcher. A failure reports the seed, trace index, case ID, and smallest
retained hand-minimized trace so the run can be replayed exactly. This version
does not claim automatic trace shrinking.

## Profile normalization

The hosted profile executes the complete corpus. `conduit-embedded` executes
the two complete normalized cases for persistent budget recovery and rejection
of an old hazardous command after local transition; it additionally exercises
the allocator-free self-support primitive without claiming the complete
self-grant-and-successor case. Every other constrained case is reported
unsupported. The physical-HIL report currently marks every attack unsupported
because the reference RP2040 fixture does not implement the required realm,
artifact, administrative, or hazardous effect boundaries. An unsupported
result is not a pass and cannot support a high-assurance release claim.

## Requirements

| ID | Obligation |
|---|---|
| ADC-001 | Retain bounded multi-step traces with explicit initial state and attacker capabilities |
| ADC-002 | Independently dispatch every named corpus trace through production validators |
| ADC-003 | Check all global containment properties after every trace step |
| ADC-004 | Reject self-support, successor self-approval, and cyclic approval |
| ADC-005 | Preserve cumulative policy budgets through recovery, replay, and identity churn |
| ADC-006 | Treat membership, transport authentication, and signatures as insufficient authority |
| ADC-007 | Reject federation laundering and public/quarantined administration |
| ADC-008 | Fail closed before consequential effects when evidence or freshness is exhausted |
| ADC-009 | Preserve inhibit state and reject stale commands, self-clear, and confinement downgrade |
| ADC-010 | Run fast deterministic traces in ordinary CI |
| ADC-011 | Run longer scheduled campaigns with reproducible seed and failing-prefix evidence |
| ADC-012 | Execute normalized constrained subsets and report unsupported HIL cases honestly |
