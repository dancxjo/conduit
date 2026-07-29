# Security policy

Conduit is an experimental research and development project. It has no stable
security-supported release and is not ready for safety-critical, autonomous,
multi-tenant, production infrastructure, or hazardous physical deployment.

The project is actively specifying containment properties for authority,
artifacts, distributed membership, plan transitions, cumulative resource use,
and physical effects. Those specifications and tests are work in progress, not
a certification or guarantee.

## Reporting a vulnerability

Please do not open a public issue for an unpatched vulnerability or include
secrets, credentials, private deployment details, or exploit material in a
public report.

Use GitHub's private vulnerability-reporting channel:

<https://github.com/dancxjo/conduit/security/advisories/new>

Include only what is necessary to reproduce and assess the issue:

- affected commit, version, crate, command, or specification;
- expected security boundary and observed behavior;
- minimal reproduction or test case;
- likely impact and required preconditions;
- whether the issue affects confidentiality, integrity, availability,
  authority, containment, enrollment, artifact admission, persistence, or
  physical effects;
- any suggested remediation or disclosure constraints.

If private vulnerability reporting is unavailable, contact the repository
owner privately and ask for a secure reporting channel before sending sensitive
details. Do not place credentials or unpublished exploit details in ordinary
email, discussions, or public issues.

No response-time or remediation-time service-level agreement is currently
offered. Reports will be handled on a best-effort basis while the project
establishes a formal security team and supported release policy.

## Public safety and design reports

Public issues are welcome for non-sensitive architectural concerns, missing
invariants, unsafe defaults, documentation problems, defense-in-depth ideas,
and test gaps that do not disclose an exploitable unpublished vulnerability.

The containment work is tracked by
[#92](https://github.com/dancxjo/conduit/issues/92) and its child issues. See
[Safety, deployment boundaries, and stewardship](docs/safety-and-stewardship.md)
for the project's current public position.

## Current support status

There are no stable supported releases or long-term security branches. The
latest commit on `main` is the only meaningful basis for a report, but it is
still experimental and may change incompatibly.

The MIT license remains the legal warranty statement. This policy provides
reporting guidance; it does not add a warranty, certification, fitness claim,
or promise that every reported issue will be fixed.

## Security boundaries to preserve

A security fix must not obtain a favorable result by weakening the architecture.
In particular:

- capability, reachability, discovery, membership, authentication, artifact
  signature, and authority remain distinct;
- a running plan cannot be the sole decisive authority for expanding its own
  authority, population, executable code base, persistence, physical reach, or
  governing budget;
- finite per-plan limits do not reset cumulative cross-epoch containment policy;
- source, semantic descriptors, exact plans, artifacts, host reports,
  authority, runtime evidence, and presentation state retain distinct
  identities;
- failure, rollback, recovery, stale observations, and missing providers fail
  closed rather than selecting a more permissive path;
- hazardous physical effects require an independently enforced local inhibit
  and safe-state boundary;
- secrets and sensitive values are represented by scoped handles and redacted
  before diagnostics, evidence projection, or presentation;
- claims about sandboxing, isolation, attestation, signatures, or provenance
  must state exactly what is and is not guaranteed.
