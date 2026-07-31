# Independent inhibit plane and hazardous-host profile current

Status: normative seed for issue #97.

## Purpose and claim boundary

Consequential physical effects require a local inhibit and safe-state boundary
that remains effective when an ordinary plan, executor, network, realm, or
mutable implementation fails. Conduit plans may request operation inside a
domain-defined envelope or request a stop. They cannot define the safe state,
expand the envelope, suppress the watchdog, replace the effect boundary, or
clear their own inhibit.

The following identities remain separate:

1. `HazardousHostProfile` is the pinned, domain-owned contract;
2. `InhibitObservation` is current host evidence about the local boundary;
3. `PlanHazardClosure` current schema binds the exact profile and observation;
4. `HazardControlState` is the host effect-boundary state machine;
5. `HazardEvidenceRecord` is immutable, bounded evidence;
6. source, resolved plan, run evidence, and Patchbay presentation retain their
   existing distinct identities.

These contracts and fixtures do not certify a physical interlock, a device, a
domain taxonomy, an operating envelope, or a complete safety case. A signature,
role, evidence label, plan node, callback, or LLM policy is not a physical
interlock. Physical HIL and domain certification remain external evidence.

## Profile and current boundary evidence

**INH-001.** A hazardous-host profile pins its descriptor, domain-owned safe
state, inhibit boundary, watchdog/interlock, host effect boundary, command and
clear effect classes, clear operation and ceremony, time basis, finite command
horizon, observation age, evidence capacity, physical-presence selection,
implementation-confinement requirement, and a finite set of exact domain-owned
envelope dimensions and signed limits. Changing any fact mints a new profile.

Core treats safe states, effect classes, operations, ceremonies, boundaries,
and envelope dimensions as pinned descriptors. It defines no robot, motor,
energy, temperature, pressure, speech, model, or UI taxonomy.

**INH-002.** An inhibit observation names the exact profile, host, safe state,
inhibit, watchdog, effect boundary, time basis, freshness interval, latch
generation/state, implementation confinement, and five enforcement facts: the
boundary is independent from the plan, has a local safe path, survives
executor loss, survives partition, and cannot be replaced by the graph.

An ordinary capability report or missing observation cannot be interpreted as
positive inhibit evidence. Resolution and run start both validate the exact
observation identity and freshness. A profile that requires isolated
implementation rejects an unconfined native implementation even when every
other label matches.

**INH-003.** Execution-current plan schema extends the current-schema hazard closure with
a bounded set of exact hazardous-host bindings. current schema identities remain
unchanged when this set is absent. A current-schema hazardous closure requires at
least one binding, rejects duplicate hosts, and revalidates each observation
at plan creation and current run-start time. The compiler seals and round-trips
the profile and observation without discovering or provisioning either.

## Arm, command, and deadman

**INH-004.** Arming is possible only from `safe-disarmed`, against a fresh
accepted host binding, the observation's exact latch generation, a nonzero
plan identity, a positive epoch, and an exact command-authority identity.
Clearing an inhibit never arms; it returns only to `safe-disarmed`.

**INH-005.** Every command binds the exact plan, epoch, authority, next sequence,
time basis, issue time, exclusive expiry, and one value for every pinned
envelope dimension. The lease horizon is finite and no larger than the
profile maximum. Future, delayed, expired, duplicate, skipped, stale-epoch,
wrong-authority, missing-dimension, duplicate-dimension, unknown-dimension,
and out-of-envelope commands are rejected before the effect boundary accepts
them.

**INH-006.** Command expiry enters the inhibited safe state. A host effect
boundary must implement the expiry without a remote round trip, plan callback,
or executor progress. `active_until_tick` is removed on inhibit.

## Inhibit, transition, and clear

**INH-007.** Stop request, command loss, lease expiry, host loss, sensor
staleness, watchdog trip, partition, authority revocation, plan transition,
implementation failure, and evidence failure may all invoke the same local
inhibit transition. The transition requires no clear authority and can only
remove capability: it drops plan, epoch, command authority, sequence, and
active lease while advancing and binding a latch.

**INH-008.** Plan replacement, rollback, reboot, firmware update, reconnect,
and realm recovery never arm a host or clear an inhibit. An armed or disarmed
host returns safe and disarmed with no old command. An inhibited host retains
the exact latch identity and generation.

**INH-009.** Clear is a separate, stricter administrative operation. It binds
the exact profile, host, latch identity and generation, protected inhibit
handle, clear effect class, clear operation, clear ceremony, administrative
subject, time basis, and current proof defined by specification 041. When the
profile selects physical presence, that current observation is additionally
required. Remote member, plan, stale proof, wrong ceremony, or wrong latch
attempts fail.

The successful result is `safe-disarmed`, never `armed`. A later arm uses a
new explicit operation and fresh boundary observation.

## Evidence, bounds, diagnostics, and testing

**INH-010.** Hosts record bounded, secret-free evidence for arm, lease and
command acceptance, command rejection, envelope limiting, inhibit cause,
safe-state entry, clear attempt, clear approval, and safe recovery. Records
bind profile, host, plan, epoch, time, sequence, predecessor, and receipt.
Exhaustion fails closed before an unevidenced protected effect.

Stable diagnostics are:

- `CND-INH-001` unsupported version;
- `CND-INH-002` invalid descriptor;
- `CND-INH-003` identity mismatch;
- `CND-INH-004` absent or stale observation;
- `CND-INH-005` independent boundary missing;
- `CND-INH-006` implementation not confined;
- `CND-INH-007` not safe to arm;
- `CND-INH-008` invalid command lease;
- `CND-INH-009` command binding mismatch;
- `CND-INH-010` command sequence invalid;
- `CND-INH-011` operating envelope exceeded;
- `CND-INH-012` command attempted while inhibited;
- `CND-INH-013` invalid clear ceremony;
- `CND-INH-014` transition attempted to clear;
- `CND-INH-015` invalid or exhausted evidence.

`conformance/c4/inhibit-plane.json` contains 29 independently dispatched
cases. The core reconstructs and executes every case. Compiler tests seal and
round-trip a current-schema hazardous-host binding and prove a later run-start
rejects its stale observation. The constrained test invokes the same local
failure and reboot hooks without allocation or a hosted service.

Those are deterministic host-oracle tests, not physical HIL. No device was
used to prove contactor state, watchdog timing, sensor coverage, safe-state
energy, firmware independence, or resistance to a malicious privileged host.
A deployment may claim the high-assurance profile only when its effect
boundary actually confines the selected implementation and its physical
evidence is reviewed outside these fixtures.
