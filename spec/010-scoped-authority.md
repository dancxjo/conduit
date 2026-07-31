# Scoped authority, effects, grants, and sensitivity current form

Status: normative current contract

Authority descriptor schema marker: `0`

## Four separate concepts

Conduit keeps four identities separate:

1. `HostCapability` is a fresh observation that a host can perform one action
   with one concrete resource.
2. `EffectRequirement` is a semantic declaration of what a node may attempt,
   including its stable requesting instance path.
3. `AuthorityGrant` authorizes one action on one exact resource under an exact
   scope, audience, constraint set, time window, delegation policy, host, and
   audit identity.
4. `ResolvedAuthorityBinding` pins one effect to the concrete capability,
   resource, host, grant, and audit identity used by an execution plan.

A capability never implies a grant. A grant does not claim that a resource is
present. A binding is not provisioning, authentication, or a native-code
sandbox.

All semantic descriptors are borrowed and allocator-free in `conduit-core`.
Effect and immutable grant descriptors have canonical semantic hashes.
Capability freshness and grant revocation are observations outside those
immutable identities.

## Effects and resources

An effect requirement contains:

- stable effect ID;
- action ID such as `audio/capture` or `filesystem/write`;
- an exact resource or resource-kind selector;
- stable requesting `InstancePath`;
- audience;
- a bounded set of domain-owned constraint descriptor references; and
- whether the grant must be checked again at each use.

A `ResourceRef` is `(kind,id)`. Kind-only selection allows deterministic host
resolution among concrete resources; exact selection cannot widen.
Constraints are references by stable ID and semantic hash. current form requires
the effect and grant constraint sets to be exactly equal, because the core
cannot safely guess domain-specific “at most” or subset meaning.
At most eight constraints are accepted by the portable current descriptor.

## Capabilities and grants

A capability observation contains capability ID, action, exact resource, host,
named monotonic time basis, observation tick, and exclusive validity-end tick.
Invalid or stale observations cannot satisfy resolution.

A grant contains:

- grant ID and action;
- exact resource;
- root instance path plus whether descendants are in scope;
- exact audience and constraints;
- named monotonic time basis;
- inclusive `not_before_tick` and exclusive `expires_at_tick`;
- host for which it was issued;
- `none`, `same-host-descendants`, or `cross-host-descendants` delegation;
- stable audit ID; and
- drain-or-abort lifecycle policy for revocation/expiry.

Revocation is a separate `GrantStatus` observation with tick and structured
reason on the grant's time basis. It does not mutate or re-hash the immutable
grant.

## Deterministic resolution

Resolution takes one `AuthorityTime` observation containing a named monotonic
time basis and tick, then validates every descriptor. It requires a fresh
capability on the selected host and a simultaneously matching active grant for
the same action and concrete resource.

A grant matches only when:

- action, resource, audience, and constraint set match exactly;
- the capability, grant, and current observation share one time basis, and the
  current deterministic tick lies in their validity windows;
- no effective revocation exists;
- requester equals the scope root or is a boundary-safe descendant permitted
  by the scope; and
- delegation permits the instance and host crossing.

If several pairs match, resolution chooses lexicographically by resource kind,
resource ID, grant ID, then capability ID. Registry or input iteration order is
not an input.

`resolve_authority_plan` resolves every placed effect into caller-owned
bindings. One denial clears bindings written by that call, so a partial plan
cannot acquire a subset of undeclared authority. Every denial preserves effect
ID, requesting path, action, and stable reason without carrying protected
value material.

Bindings marked `check_at_use` are revalidated against the exact pinned
capability and grant. Substitution of another grant, resource, host,
capability, or audit identity is a binding mismatch.

## Composite and distributed authority

A composite aggregates every reachable child effect in deterministic child
order. Exports cannot hide internal effects, and a composite boundary grants
no authority. Caller-provided finite storage bounds aggregation.

Each child effect retains its expanded requesting path. A grant rooted at the
composite may authorize descendants only when both scope and delegation say
so. Moving a child to another host additionally requires
`cross-host-descendants`; host capability on the remote host is still checked
separately. Distributed transport does not broaden action, resource, audience,
constraints, or time.

Plan-pinned replicated children from #44 receive separate instance/attempt
paths and authority slices. Restarts never reuse a binding whose scope,
audience, time, or attempt identity no longer matches.

## Revocation, expiry, and evidence

Expiry and revocation fail use-time validation. They map respectively to
`deadline-expired` and `authority-revoked` lifecycle causes and apply the
grant's exact drain-or-abort policy. Specification 008 terminal precedence
then resolves any simultaneous failure or cancellation.

Immutable authority evidence payloads cover binding, successful use, denial,
revocation, and expiry. They retain requesting path, action, grant ID, and
audit ID where known. Specification 012 supplies the common provenance
envelope and sequence ownership.

The core models authority but cannot make native in-process code safe. A host
integration must enforce the binding at the implementation/resource boundary.
No resolver provisions hardware, prompts for login, changes host
configuration, or manufactures a grant.

## Sensitivity

The existing total order is:

```text
public < restricted < secret
```

Connections never cross into a weaker destination, even with a presentation
or recording grant. Declassification remains an explicit authorized adapter.

| Use | Public | Restricted/secret |
|---|---|---|
| connect | value if destination ceiling accepts it | value only if ceiling accepts it |
| record | value | value only with matching `conduit/data.record` binding and sufficient ceiling |
| present | value | value only with matching `conduit/data.present` binding and sufficient ceiling |
| diagnostic | value | always redacted |
| evidence | value | always redacted |

`EvidenceValue::Redacted` contains only sensitivity, TypeContract reference,
and whether the value existed. It has no field capable of carrying bytes.
Redaction therefore preserves stable metadata and presence without depending
on formatter discipline. Hosted `SecretValue` also redacts ordinary Debug and
Display; explicit exposure remains confined to an authorized implementation
boundary.

## Diagnostics and fixtures

| Code | Meaning |
|---|---|
| `CND-HST-001` | required fresh host capability is missing or stale |
| `CND-AUT-001` | no grant exists for the effect |
| `CND-AUT-002` | grant action/resource/scope/audience/constraints/time/delegation mismatch |
| `CND-AUT-003` | pinned grant expired or was revoked |
| `CND-AUT-004` | authority descriptor is malformed or exceeds a portable bound |
| `CND-AUT-005` | use-time facts do not match the pinned binding |
| `CND-AUT-006` | caller binding or aggregation storage is too small |

`conformance/c2/authority.tsv` freezes allow, missing-grant denial,
resource/scope mismatch, expiry, cross-host non-delegation, sensitivity
downgrade, composite aggregation, and redacted evidence.

Reference tests also prove deterministic selection, capability/grant
separation, all-or-nothing plan resolution, use-time revocation, lifecycle
mapping, canonical identity changes, complete cause-safe denials, and protected
evidence construction.

## Normative requirements

| ID | Obligation |
|---|---|
| AUT-001 | Never treat capability as permission |
| AUT-002 | Bind every effect to one concrete resource and active exact grant |
| AUT-003 | Reject scope, audience, constraint, time, and delegation mismatch |
| AUT-004 | Resolve every plan effect or produce no partial authority plan |
| AUT-005 | Revalidate pinned facts at use time when required |
| AUT-006 | Aggregate every reachable composite effect regardless of exports |
| AUT-007 | Require explicit cross-host delegation for distributed use |
| AUT-008 | Retain stable audit and requesting-path identity in evidence |
| AUT-009 | Map revocation and expiry into exact lifecycle causes |
| SEN-001 | Never permit an implicit sensitivity downgrade |
| SEN-002 | Require explicit authority to record or present protected values |
| SEN-003 | Make diagnostic and evidence redaction structural, not formatter-only |
