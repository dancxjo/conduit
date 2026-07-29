# Safety, deployment boundaries, and stewardship

Conduit is a general-purpose composition and execution project. Its typed nodes,
bounded cords, heterogeneous host resolution, exact plans, authority, artifacts,
distributed transport, live transitions, and evidence can describe useful
systems across Linux, browsers, embedded devices, and remote hosts.

That generality is dual-use. Compositions involving discovery, enrollment,
artifact activation, delegation, persistence, distributed placement, network
boot, or physical effects can create system-level behavior that is not visible
when each operation is reviewed alone.

This document records the project's current public boundary. It is intentionally
plain about what exists, what is still being designed, and what must not be
claimed.

## Experimental status

Conduit is not currently a production operating system, deployment platform,
fleet manager, public compute realm, autonomous infrastructure controller, or
certified safety system. The repository contains an executable foundation,
candidate specifications, conformance fixtures, examples, and an evolving
roadmap.

The following are not current project guarantees:

- containment of hostile multi-tenant workloads;
- secure autonomous enrollment or federation;
- safe unattended installation or self-update;
- safe network boot or bare-metal fleet orchestration;
- resistance to a compromised administrator, host, firmware, provider, or
  physical boundary;
- suitability for hazardous actuation;
- real-time or high-availability guarantees;
- formal verification, certification, or independent security audit.

A specification, test fixture, exact plan, signature, provenance record,
capability report, or evidence stream is not by itself proof of safe deployment.

## Cross-cutting containment rule

The safety program is organized around this invariant:

> A running Conduit system must never increase its own authority, population,
> executable code base, persistence, physical reach, or governing budget solely
> through actions whose decisive authorization it already controls.

Local checks remain necessary but are not sufficient. A sequence such as:

```text
discover host
+ enroll host
+ acquire or delegate authority
+ install or activate an artifact
+ start a successor plan
+ repeat
```

can amplify a system even if every individual plan has finite queues and every
individual operation presents a valid grant. Containment therefore has to
survive runs, plan epochs, generations, rollback, reboot, recovery, and realm
changes.

The owning design program is
[#92](https://github.com/dancxjo/conduit/issues/92), with work covering
administrative-plane separation, persistent proliferation budgets, whole-plan
hazard closure, safe realm genesis and distribution, independent inhibit
planes, and adversarial conformance.

## Dangerous capability gates

The project should not ship convenient reference implementations or examples
for the following capabilities until their owning containment contracts and
negative tests are in place:

- autonomous realm creation, enrollment, federation, or trust expansion;
- workload-controlled grant creation, delegation, or administrative approval;
- unattended artifact installation, persistence, or successor activation;
- network boot or bare-metal provisioning of additional machines;
- recursive discovery-and-enrollment loops;
- default public realms, global discovery, or open administrative endpoints;
- recovery paths that weaken identity, authority, security, or resource policy;
- physical actuation without an independent local inhibit and defined safe
  state;
- self-update that can remove its controlling or recovery path;
- provider registries that make dangerous administrative effects available by
  default.

Absence is stronger than a default-off checkbox. Dangerous administrative
providers should be excluded from reference distributions until deliberately
installed and configured by an external operator under an exact policy.

Research prototypes in these areas must be isolated, explicitly bounded, and
clearly labeled. They must not be presented as deployment guidance.

## Roles that must remain separate

Conduit design and documentation must not collapse these distinctions:

- discovery is not enrollment;
- reachability is not authority;
- membership is not a grant;
- authentication is not authorization;
- a signed artifact is not a confined artifact;
- a host capability is not permission to use it;
- an authored panel is not an admitted plan;
- a candidate plan is not the active plan;
- self-observation is not administrative authority;
- rollback is not permission to select a weaker policy;
- evidence is not an authorization source;
- presentation or Patchbay state is not runtime truth;
- transport security is not application or realm authority;
- per-plan finiteness is not a cumulative proliferation bound.

These distinctions apply equally to CLI, Patchbay, browser, embedded, remote,
and headless implementations.

## Public source and responsible disclosure

Keeping the specifications and general implementation public permits independent
review, portable implementations, reproducible conformance work, and early
identification of unsafe compositions. The project does not rely on secrecy as
a containment mechanism.

Public development does not require immediate publication of every exploitable
detail. Unpatched vulnerabilities, credentials, private deployment data, and
turnkey exploit material should follow [SECURITY.md](../SECURITY.md) and remain
private until a responsible disclosure decision is made.

The project may postpone or omit dangerous convenience tooling even when the
underlying general-purpose primitives are public.

## Deployment guidance

Until a supported security profile exists:

- use Conduit only in controlled experimental environments;
- do not expose control, enrollment, artifact, realm, or Patchbay
  administrative endpoints to untrusted networks;
- do not grant ambient filesystem, process, secret, network, installation,
  persistence, or device authority;
- do not connect experimental plans to hazardous physical equipment;
- use deterministic fakes for enrollment, network, failure, and physical-effect
  examples;
- keep an independent recovery and shutdown path outside the running plan;
- pin exact artifacts and inputs, but do not treat pinning as sandboxing;
- preserve bounded, redacted evidence for review;
- assume host, firmware, boot, and provider compromise remain outside current
  guarantees;
- perform an independent threat model and review before any consequential use.

## Stewardship direction

Conduit is currently maintained through this repository; it does not yet have
a foundation, security council, formal technical steering committee, supported
release team, or public trust infrastructure.

If the project gains adopters, stewardship should move toward plural,
transparent, public-interest governance:

- multiple independent maintainers and reviewers;
- documented decision and compatibility processes;
- a private security response team and disclosure policy;
- independent security and safety review;
- representation from embedded, distributed-systems, accessibility, robotics,
  security, and public-interest communities;
- participation by companies, universities, and public institutions without
  unilateral control by any one of them;
- separation of specification governance from artifact signing, distribution,
  hosted services, realm administration, and trust roots;
- no universal Conduit root key, mandatory public realm, or master
  administrative service.

This is a direction, not a claim that such governance already exists.

## Contributions

Safety contributions are welcome. Useful work includes:

- cross-epoch and whole-plan threat models;
- stable negative reason codes and fail-closed behavior;
- adversarial multi-step conformance cases;
- bounded recovery and rollback proofs;
- authority and sensitivity review;
- deterministic hostile-provider fixtures;
- documentation that narrows overbroad claims;
- analysis of human administration and recovery failure;
- independent implementations that expose specification ambiguity.

Do not weaken a required invariant merely to make an example resolve or a test
pass. When a policy decision belongs to an unresolved safety issue, preserve an
explicit typed seam, document the dependency, and return an honest unsupported
or denied result.
