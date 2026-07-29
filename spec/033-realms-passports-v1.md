# Realms, entity passports, and membership lifecycle version 1

Status: candidate normative C2/C4 contract. This specification is additive to
the frozen authority, evidence, Resonance, artifact, host-report, and plan
schemas. `conformance/c2/realms-passports-v1.json` is the normative fixture.

## Identity boundary

The following identities are independently named and never substitute for one
another:

1. A **realm** is a stable, authenticated administrative and causal namespace.
2. An **entity** is a stable host, service, user, agent, or domain-owned part.
3. A **key** is a rotatable public verification identity associated with a
   realm or entity.
4. A **membership credential** is a realm-issued, bounded statement about an
   entity/key pair.
5. A **role binding** is a replaceable policy assignment.
6. A **workload delegation** is a bounded identity for one plan/run/epoch.
7. An **authority grant** permits an exact operation; membership and role do
   not create it.
8. A **capability report** is fresh evidence of current host facts.
9. An **artifact signature** vouches for bytes, not for the executing entity.
10. **Transport authentication** identifies a carrier peer/session, not a
    Conduit effect grant.

A callsign, display name, current key fingerprint, current role, reachable
endpoint, artifact signer, TLS identity, or host report is insufficient proof
of realm or entity identity (`RLM-001`). Private key bytes are never accepted
by a realm descriptor, passport, panel, plan, event, diagnostic, or
presentation model (`RLM-002`). Implementations keep signing keys behind an
opaque host handle.

## Immutable descriptors and fresh observations

`RealmDescriptor` canonically pins its stable realm ID, genesis root binding,
accepted root keys/epochs, policy descriptor, membership/delegation/event
integrity/revocation/federation profiles, and finite maxima for root anchors,
succession records, members referenced by a resolution, and delegation depth.
The display callsign is presentation metadata and excluded from security
identity (`RLM-003`). A realm root succession is an immutable signed record
which names predecessor/successor key IDs and epochs; it must be bounded,
acyclic, ordered by epoch, and trace from the accepted root to genesis
(`RLM-004`). An emergency replacement is a separately declared policy path;
conflicting root views fail closed (`RLM-005`).

`EntityPassport` canonically pins entity ID, domain-owned profile descriptor,
issuer realm, enrollment provenance, public-key references, membership
credential, role bindings, key-protection/attestation descriptors, privacy
sensitivity, and bounded typed extension descriptors. It contains no key
bytes and is not a mutable current-status record (`RLM-006`). Domain contracts
may describe a physical part, calibration, or body relationship through a
typed extension; core has no organism, body-part, vendor, or role-name enum
(`RLM-007`).

`PassportStatusObservation` is separately fresh and names the exact passport,
realm, entity, status source, named time basis, observation tick, expiry, current
membership/key state, and bounded revocation/gap outcome. Safety/authority
sensitive verification fails closed if current required status is stale,
unavailable, gapped, suspended, revoked, retired, or compromised
(`RLM-008`). An explicit offline profile may accept a pinned credential only
until its declared expiry and must emit the weaker-status outcome (`RLM-009`).

## Membership, keys, and roles

Enrollment is an explicit authorized effect: candidate identity, proof of key
control, domain ceremony/attestation, narrow authorization, credential issue,
immutable control evidence, then a fresh status/passport projection. Discovery,
resolution, browser permission, connection, artifact loading, or transport
scouting never enrolls a member (`RLM-010`).

Membership credential validity is bounded by realm/key/entity IDs, issuance
and expiry, allowed audiences, role/delegation constraints, issuer key/epoch,
and a signature or authenticated-recorder receipt reference. The core validates
these pins and bounds; a selected crypto provider verifies actual signatures
and exposes only a verification result/reference (`RLM-011`). A signature is
not proof of truth, safety, authorization, confidentiality, freshness, or
confinement (`RLM-012`).

`CredentialVerification` is that identified, bounded provider result. It pins
the exact credential and passport identities, selected verifier, challenge,
time basis and validity interval, outcome, and receipt. Rejected and replayed
challenges fail as credential verification; conflicting live sessions and an
unavailable provider fail as current status. `validate_passport_at` combines
static passport validation, credential lifetime, and this provider result
without exposing key bytes or performing the challenge itself.

Key rotation preserves an entity ID only through an authorized immutable key
transition. Root rotation likewise preserves realm ID only through a valid
succession record. Replacement hardware receives a new entity ID even when it
is assigned an old role. Role reassignment never rewrites old authorship
(`RLM-013`). A software-held/exportable key may be enrolled only with an
honest key-protection/attestation descriptor; it does not claim hardware
protection or clone resistance (`RLM-014`).

`EntityKeyTransition` names the stable entity, prior/successor key IDs and
epochs, authorizing credential, and receipt. Validation requires consecutive
epochs and keys present in the exact prior/successor passport views.

## Event authorship and delegation

`EventAuthorship` extends a Resonance envelope without modifying frozen
`ExecutionEvent` identity. It pins issuer realm, producer entity, optional
workload delegation, signing key/credential, exact integrity profile,
signature/batch/recorder receipt reference, verification outcome, status
freshness outcome, and optional gateway/bridge identity. Direct signature,
signed batch, and recorder receipt are distinct attribution strengths
(`RLM-015`). A bridge preserves original realm/entity/key authorship and
causation, while separately naming its own receipt; it never rewrites a remote
event as local authorship (`RLM-016`). Passport data is sensitive by default;
evidence carries approved stable references and redacted-presence metadata,
not whole credentials or extensions (`RLM-017`).

`WorkloadDelegation` is a short-lived signed/verified binding from an enrolled
entity to one realm, exact plan/run/epoch, optional semantic subject, audience,
actions/resources/streams, expiry/status, and finite delegation depth. It may
not exceed its membership or grant bounds and is never an ambient grant
(`RLM-018`). At operation time the existing exact `AuthorityGrant` remains
required (`RLM-019`).

## Federation

`FederationPolicy` is an immutable, signed, directional relationship between
exact local and remote realm/root epochs. It independently pins allowed
entity/profile/role predicates, audiences, actions, resources, streams, event
classes, bridge identities, expiry/status freshness, and whether identity,
event verification, transport admission, or grant delegation is allowed.
Federation is explicit, scoped, expiring, revocable, and non-transitive;
`A -> B` plus `B -> C` never implies `A -> C` (`RLM-020`). It is not a global
PKI, universal realm, or trust-on-first-use scheme.

## Deterministic verification

Realm/passport verification consumes only immutable descriptors plus explicit
key/credential signature-verification results, status observations,
federation policies, grants, and a named time observation. Selection is
canonical; discovery order, network state, host mutation, ambient clock, and
browser/user-agent state are invalid inputs (`RLM-021`). Inputs and output
reason trees are bounded by the selected profile. Resolution can bind a
passport/status/delegation to a plan or reject it; it does not enroll, rotate,
revoke, fetch, prompt, discover, connect, sign, or provision (`RLM-022`).
Capability-report schema 2 consumes this boundary through an identified
realm/entity/passport/status binding. Resolver policy independently selects
required realms, trusted entities, and trusted status reporters; successful
membership validation still does not create an authority grant.

## Control events and presentation

Enrollment requested/challenged/approved/denied; credential issue/renewal/
expiry; key add/rotate/compromise/retire; role bind/unbind/reassign; entity
suspend/reinstate/revoke/retire; root add/rotate/emergency-replace/retire;
federation establish/narrow/suspend/revoke; and passport projection rebuild,
stale, or gap are domain/control Resonance events. They append; they never
mutate history (`RLM-023`). Presentation may show a redacted passport view or
callsign but neither contributes to a semantic identity (`RLM-024`).
`RealmControlEvent` requires a control-class Resonance envelope, exact
recording authority, non-public sensitivity, and non-inline protected payload.
Hosted passport inspection returns stable realm/entity/profile/credential
references while structurally omitting key digests, receipts, roles, and
extensions.

## Stable reasons

- `CND-RLM-001` malformed or unsupported realm/passport descriptor
- `CND-RLM-002` realm/passport/delegation identity mismatch
- `CND-RLM-003` invalid root/key succession or conflicting root view
- `CND-RLM-004` credential or signature verification rejected
- `CND-RLM-005` status missing, stale, gapped, revoked, suspended, or retired
- `CND-RLM-006` membership/role/delegation scope or expiry mismatch
- `CND-RLM-007` federation absent, expired, denied, or non-transitive
- `CND-RLM-008` private/sensitive passport material attempted at a public boundary
- `CND-RLM-009` required exact authority grant is absent or mismatched
- `CND-RLM-010` bounded storage/scratch/delegation/anchor limit exceeded

## Normative requirements

| ID | Obligation |
|---|---|
| RLM-001 | Keep realm, entity, key, membership, role, workload, grant, capability, artifact, and transport identities distinct. |
| RLM-002 | Keep private key bytes out of Conduit contracts and presentation. |
| RLM-003 | Make realm/entity identity stable independently of callsigns/current keys. |
| RLM-004 | Validate bounded, verifiable root succession. |
| RLM-005 | Reject conflicting root continuity deterministically. |
| RLM-006 | Model passports as immutable bounded current views with typed extensions. |
| RLM-007 | Keep physical/domain semantics outside core. |
| RLM-008 | Make membership/revocation status fresh and fail closed where required. |
| RLM-009 | Permit only explicit bounded offline weakening. |
| RLM-010 | Make enrollment an authorized observable effect. |
| RLM-011 | Bind membership to verifiable credential/key facts without embedding crypto keys. |
| RLM-012 | Do not overstate what signatures establish. |
| RLM-013 | Preserve identity/authorship across rotation and replacement correctly. |
| RLM-014 | Declare attestation and key-protection limits honestly. |
| RLM-015 | Preserve exact event attribution strength. |
| RLM-016 | Preserve remote authorship through bridges. |
| RLM-017 | Redact passport data structurally. |
| RLM-018 | Bound and pin workload delegation. |
| RLM-019 | Never turn membership or a role into an ambient action grant. |
| RLM-020 | Make federation explicit, directional, scoped, revocable, and non-transitive. |
| RLM-021 | Resolve only explicit deterministic evidence. |
| RLM-022 | Prohibit implicit enrollment/provisioning/mutation during verification. |
| RLM-023 | Preserve append-only lifecycle control evidence. |
| RLM-024 | Keep presentation outside security identity. |
