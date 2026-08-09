# Durable system continuity

Durable continuity is a realization record above current host reports, exact
Plans, and Plays. It is not a runtime, scheduler, planner, discovery service,
membership database, or authority issuer.

The `no_std` `conduit-system-continuity` contract consumes a validated
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
  -> old boot termination Sign
  -> distinct replacement boot report
  -> compatible offers may be identified
  -> old assignments and boot-scoped grants are stale
  -> explicit newly sealed Plan and new Plays are required
```

Request acceptance cannot be used as termination or replacement proof. A report
reusing the old boot fails. A face-compatible offer on the new boot is only a
candidate: continuity resumes only after a different exact Plan assigns the
role to that boot, stale grants are absent, and Play identities are new.

## Optional delegated reboot

Delegated reboot is an optional exact operation offer, not host core. One
externally issued `DelegatedTransitionGrant` names the exact controller and
target host boots, selected equal-face capability realization, admitted session
link, maximum attempts, proof window, and host-reserved Sign sequence range.
The bounded transaction consumes that existing grant fact; it does not issue
authority or create another authority store.

Admission independently checks current target advertisement, canonical
checked-face compatibility, exact selected capability, controller and target
boots, and the existing validated `SessionBinding`. Unsupported, unauthorized,
stale, malformed, replayed, exhausted, and wrong-session requests remain
distinct machine-readable denials. Acceptance Sign is not completion:
old-boot terminal Sign and a distinct available replacement boot report are
both required. Loss of the admitted control Line after acceptance remains a
pending intentional transition until that correlated proof arrives or the
finite proof window expires to `UnknownProofWindowExpired`.

The first conformance proof consumes the already accepted std/browser/Pico
Signal arrangement as software facts. It does not rerun or enlarge the physical
S4 claim. No host category appears in the continuity rules; other conforming
host compositions participate through the same advertisements, Plans, and
reports.

## Stop line

This layer grants no authority and performs no platform reboot. It validates and
records the first delegated-reboot transaction around an external exact grant.
It does not keep a mutable fleet truth table or revive `conduit-realm`.
Discovery, cryptographic identity, orchestration, OTA/update staging,
installation, Play start, rollback, and generalized durable-host grants remain
outside this proof.
