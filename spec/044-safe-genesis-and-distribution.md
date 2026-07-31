# Safe realm genesis and reference distributions current

Status: normative seed for issue #96.

## Purpose and claim boundary

A fresh, reset, or partially recovered Conduit installation must not acquire
ambient authority merely because it has joined a network, opened a browser,
learned a callsign, received a capability report, or discovered another host.
This specification defines allocator-free facts for a fail-closed realm
genesis, member quarantine, explicit public profiles, reference distribution
defaults, deliberate provider enablement, and monotonic recovery.

The following identities remain distinct:

1. a `RealmGenesisProfile` is the sealed semantic policy for one genesis class;
2. a `GenesisStateObservation` is a current host observation validated against
   that profile;
3. a `GenesisControlRecord` is immutable, secret-free evidence;
4. a `ReferenceDistributionProfile` is an exact package/provider inventory;
5. a `.panel`, resolved execution plan, run evidence, and Patchbay projection
   retain their existing separate identities.

A friendly name, realm name, callsign, profile label, browser origin, network
address, or distribution filename is not a security identity. A checked-in
distribution document proves the intended reference inventory and its
validation result. It cannot prove that an operating-system package manager,
browser extension store, physical debugger, bootloader, or third-party image
cannot install additional software. Those controls remain host and deployment
responsibilities.

## Safe genesis profiles

**GEN-001.** A genesis profile pins its canonical descriptor, class, disabled
or simulation-only safe plan, optional exact local-bootstrap realm and
identity, separate bootstrap authorizer and evidence recorder, exact recovery
effect class and operation, time basis, finite ceremony lifetime and retry
limit, finite evidence capacity, and bounded public-operation set. Every
identity-bearing descriptor and set member participates in the profile
identity. Changing any one mints a new profile.

`conduit-core` recognizes private, shared-private, deliberately-public,
simulation-only, and constrained-offline classes. These names select generic
validation rules only; domains retain ownership of realm and operation
meanings.

**GEN-002.** An unconfigured state has no members, realm authority, grants,
federations, dangerous provider, discovery, public listener, or unrestricted
network. It runs only the profile's exact disabled or simulation-only safe
plan. If a local bootstrap identity is configured, the local-bootstrap state
may contain only that exact realm and at most that identity, still without
roles, grants, delegation, federation, providers, subscriptions, remote plan
activation, administrative effects, or actuating effects.

Network attachment is not an input that can mutate this state. The hostile-LAN
fixture validates the unchanged post-observation state; it does not model the
network as a trusted bootstrap channel.

## Local bootstrap and quarantine

**GEN-003.** Bootstrap requires one exact, locally confirmed ceremony using a
profile-pinned physical-presence, USB, BLE, or temporary-local channel and the
exact bootstrap authorizer. The attempt pins candidate identity and key,
profile, channel, time basis, inclusive issue time, exclusive expiry, retry
ordinal, replay and remote-session facts, and a receipt. The evidence recorder
must bind the same profile, candidate, time basis, time, and receipt.

**GEN-004.** Network attachment, browser navigation, PWA installation, browser
permission, transport handshake, capability report, and callsign observation
never enroll a member. They fail with `CND-GEN-005`, even if a surrounding
host or UI regards the event as trusted.

**GEN-005.** A valid bootstrap produces only
`MemberDisposition::Quarantined`. A quarantined passport has no role, grant,
delegation, federation, provider, protected subscription, remote plan
activation, administrative effect, or actuating effect. Later membership,
role, grant, delegation, and administrative operations use their own exact
contracts and evidence. Quarantine is not an implicit small grant.

**GEN-006.** Ceremony lifetime, retry count, replay status, local confirmation,
control-record identity, and evidence capacity are checked before enrollment.
Control records are immutable and secret-free. Their receipt is a provider
observation; the selected recorder is responsible for binding it to the exact
ceremony and for durable predecessor storage. Sequence one has no predecessor;
every later record must name one, and a record cannot name itself.

## Deliberately public profiles

**GEN-007.** A deliberately-public profile exposes only its finite set of
exact domain-owned operations, each with a positive maximum-use count. Generic
traits prohibit using this set for administration, deployment, protected
subscription, or actuation. Private, shared-private, simulation-only, and
constrained-offline profiles expose no public genesis operation.

Public operation admission is not a grant, plan activation, subscription, or
provider enablement. The host still applies ordinary authority, policy budget,
and execution contracts to any later work.

## Reference distribution defaults

**GEN-008.** A reference distribution pins its hosted, browser, or constrained
kind; genesis profile; separate control recorder; exact provider-enablement
effect class and operation; finite providers; and bounded enablement time,
retry, and evidence limits.

Every provider carries an exact descriptor, optional immutable artifact,
availability (`absent`, `disabled`, `enabled`, or `unsupported`), and generic
risk traits for enrollment issuance, unrestricted native execution, remote
artifact installation, firmware mutation, unrestricted network access, realm
root administration, remote plan activation, and actuating effects.

**GEN-009.** A reference distribution is invalid if any provider with one or
more dangerous traits is enabled by default. The checked-in hosted, browser,
and constrained profiles enumerate those provider classes as absent, disabled,
or unsupported. Browser and constrained profiles report genuinely unsupported
capabilities as unsupported instead of pretending to implement them.

An enabled provider with no dangerous trait is an inventory fact, not a
capability grant. Provider availability is a compile input and diagnostic
boundary; it does not enter execution-plan identity or substitute for exact
implementation, artifact, host-observation, passport, or authority bindings.

**GEN-010.** An exact provider requirement succeeds only for the same pinned
provider with all required traits and `enabled` availability. Absent,
disabled, unsupported, missing, and trait-mismatched providers are distinct
selection results. `conduct --check` and `conduct --explain` report the exact
provider and declared availability without installing, enabling, or fetching
anything.

## Deliberate provider enablement

**GEN-011.** Enabling a dangerous provider requires the exact distribution,
provider, immutable artifact, time basis, bounded ordinal and lifetime,
immutable control record, and an independent administrative-containment proof
from specification 041. The proof must name the distribution's exact
provider-enablement class and operation and bind the provider and artifact in
its administrative subject.

Provider enablement carries an empty effect-grant collection. Installing or
enabling code never authorizes that code's effects. Persistent consumption of
a policy budget, when required by the domain policy, remains the host ledger
operation defined by specification 042.

## Reset and recovery

**GEN-012.** Factory reset, lost-root recovery, and failed restore return to the
fully isolated authority surface. They cannot carry an approval or snapshot
that silently preserves authority.

**GEN-013.** Restore, rollback, and emergency recovery require the exact
reviewed snapshot, authority ceiling, recovery class and operation, profile,
and independent administrative proof. Candidate membership, grants,
delegations, federation, executable providers, root authorities, remote
activation, protected subscriptions, actuation, discovery, listener, network,
ambient-root, and trust-on-first-use facts must each be no wider than the
reviewed ceiling. Emergency recovery cannot create a universal root.

Recovery validation authorizes no effect and performs no persistence. A host
must separately record and execute the transition.

## Bounds, diagnostics, and conformance

**GEN-014.** The reference validators use caller-provided fixed storage.
Bootstrap channels, public operations, members, member authority collections,
providers, retries, lifetimes, evidence, and canonicalization work are all
finite. Storage exhaustion fails closed.

Stable diagnostics are:

- `CND-GEN-001` unsupported version;
- `CND-GEN-002` invalid descriptor;
- `CND-GEN-003` identity mismatch;
- `CND-GEN-004` unsafe initial state;
- `CND-GEN-005` implicit or remote bootstrap;
- `CND-GEN-006` expired, replayed, or retry-exhausted bootstrap;
- `CND-GEN-007` invalid or exhausted evidence;
- `CND-GEN-008` quarantine violation;
- `CND-GEN-009` denied public operation;
- `CND-GEN-010` absent, disabled, or unsupported provider;
- `CND-GEN-011` dangerous provider enabled by default;
- `CND-GEN-012` invalid or unapproved provider enablement;
- `CND-GEN-013` widened recovery;
- `CND-GEN-014` storage bound exceeded.

`conformance/c4/safe-genesis.json` contains 31 independently dispatched
cases. The core test reconstructs and executes every case rather than
recognizing identifiers. Checked-in distribution tests independently parse,
reseal, and validate all three reference profiles. The embedded test validates
the constrained defaults. The browser engine executes navigation, PWA,
permission, transport, and capability-report signals and requires
`CND-GEN-005`.

These fixtures prove the portable decisions for supplied facts. They do not
prove physical-presence hardware, browser-store policy, third-party package
exclusion, durable host storage, cryptographic receipt generation, deployment
configuration, or a complete domain effect taxonomy. Issue #88 retains the
broader live browser and deployment realm/passport proof boundary. Issue #57
owns live transition invocation, and issue #97 owns runtime inhibition.
