# Confined hosted gear profile

Status: implemented hosted mechanism and adversarial proof for issue #3075.

The `conduit-std-host` `confined-gear` feature admits WebAssembly artifacts to
a no-WASI Wasmi engine. Preparation verifies the SHA-256 artifact identity,
records every import, rejects imports outside the selected profile, and
requires one fixed-size exported linear memory. Execution applies finite
artifact, memory, input, output, fuel, instance, table, and host-call bounds.
Instruction fuel is a hard Host boundary: the adversarial fixture contains an
actual infinite guest loop with no cooperative callback, and Wasmi returns
control as `InstructionFuelPreempted` when the admitted grant is consumed. This
is forced containment, not a wall-clock timeout and not a semantic continuation.

Pure artifacts receive no imports. In particular, the profile supplies no
filesystem, network, subprocess, device, credential, browser, ROS, clock, or
randomness surface. Effectful artifacts may receive only
`conduit.effect_call`. Its trusted closure validates a finite guest-memory
frame and an exact opaque slot, operation tag, and resource tag before asking
the host-owned `BaseCapabilityTable` to authorize one operation. The guest
cannot read or modify the capability bearer or table. Revocation therefore
invalidates a later call even when the module retains every descriptive ID.

## Trust and inspection

The Wasmi engine, this profile's import closure, the capability table, and the
selected base provider remain trusted native code. A confined artifact is not
trusted merely because it has a `.wasm` suffix: its digest and import inventory
must have produced a `PreparedArtifact`. The inspection class is one of:

| Class | Meaning shown by Patchbay/Observatory consumers |
| --- | --- |
| `ConfinedThirdParty` | Artifact executes behind this checked no-WASI boundary. |
| `TrustedNative` | Native host/base implementation is deliberately trusted. |
| `CooperativeNative` | Native code shares process privilege; semantic checks are not hostile-code confinement. |

These classes are not interchangeable security badges. Browser JavaScript is
outside this proof and may independently possess browser authority. The proof
does not claim native-code, kernel, DMA, side-channel, wall-clock deadline, or
physical isolation.

## Proof and stop line

Run `cargo xtask check confined-gear`. The suite executes the production
engine/import provider with pure and effectful modules and hostile fixtures for
forbidden and undeclared imports, wrong digest, growable memory, fuel
exhaustion, host-call flooding, forged slots, sibling resources, revoked
capabilities, and malformed guest ranges. forms remain semantic documents and
contain no WebAssembly, runtime, ABI, import, digest, or capability-slot facts.

There is no WASI preopen, general host adapter, base registry access, network or
shell convenience import, or automatic retry. Engine fuel exhaustion, import
refusal, malformed memory, capability refusal, base refusal, and execution
failure remain distinct machine-readable dispositions.

## Portable Step law

The execution kernel applies one portable law across targets: the plan owns a
finite fuel grant for each Step; a cooperative Back spends that fuel through
`StepIo`, returns one distinct outcome, and keeps long-lived computation in
explicit Back state between Steps. Grant, cooperative yield, and exceeded-grant
evidence are separate Signs. Cancellation, pressure, Host Call wait/completion,
Back failure, and semantic completion remain separate outcomes.

Only this hosted Wasm profile currently admits non-cooperative third-party Back
code. Hosted native Backs share process privilege and are cooperative. Browser,
ConduitOS, and firmware execution use the same bounded-Step scheduler today but
do not claim Worker termination, timer preemption, task isolation, or watchdog
continuation until an exact target profile implements and proves it. Those Hosts
must not admit an untrusted Back that can ignore cooperative fuel.
