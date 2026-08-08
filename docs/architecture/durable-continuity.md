# Durable system continuity

Durable continuity is a realization record above current host reports, exact
Plans, and Plays. It is not a runtime, scheduler, planner, discovery service,
membership database, or authority issuer.

The allocator-free `conduit-system-continuity` contract consumes a validated
Observatory snapshot. Callers separately declare membership and checked-face
role requirements. Construction succeeds only when each role maps to one exact
planned placement, one explicitly available host+boot offer, and one matching
Play. The assignment retains exact capability, implementation, artifact,
placement, host, and boot identity. Equal checked faces establish functional
compatibility; they never transfer assignment or proof provenance.

## Distinct facts

The record keeps these facts separate:

- membership is explicitly supplied and is not inferred from a host report;
- observed links are retained as observations and do not imply membership;
- capability availability is checked independently of either;
- boot-scoped authority comes from the exact Plan;
- delegated transition grants are external facts that this layer can validate
  but never issue;
- Plan identity fixes one realization, while Play identities name executions.

## Replacement sequence

Replacement is deliberately staged:

```text
local request or externally authorized request accepted
  -> old boot termination evidence
  -> distinct replacement boot report
  -> compatible offers may be identified
  -> old assignments and boot-scoped grants are stale
  -> explicit newly sealed Plan and new Plays are required
```

Request acceptance cannot be used as termination or replacement proof. A report
reusing the old boot fails. A face-compatible offer on the new boot is only a
candidate: continuity resumes only after a different exact Plan assigns the
role to that boot, stale grants are absent, and Play identities are new.

The first conformance proof consumes the already accepted std/browser/Pico
Signal arrangement as software facts. It does not rerun or enlarge the physical
S4 claim. No host category appears in the continuity rules; other conforming
host compositions participate through the same advertisements, Plans, and
reports.

## Stop line

This layer grants no reboot/update authority and performs no transition. It
does not keep a mutable fleet truth table or revive `conduit-realm`. Discovery,
cryptographic identity, orchestration, package installation, and generalized
durable-host grants remain outside this proof.
