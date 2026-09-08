# Confined hosted Gear profile

Status: implemented hosted mechanism and adversarial proof for issue #3075.

The `conduit-std-host` `confined-gear` feature admits WebAssembly artifacts to
a no-WASI Wasmi engine. Preparation verifies the SHA-256 artifact identity,
records every import, rejects imports outside the selected profile, and
requires one fixed-size exported linear memory. Execution applies finite
artifact, memory, input, output, fuel, instance, table, and Host-call bounds.

Pure artifacts receive no imports. In particular, the profile supplies no
filesystem, network, subprocess, device, credential, browser, ROS, clock, or
randomness surface. Effectful artifacts may receive only
`conduit.effect_call`. Its trusted closure validates a finite guest-memory
frame and an exact opaque slot, operation tag, and resource tag before asking
the Host-owned `BaseCapabilityTable` to authorize one operation. The guest
cannot read or modify the capability bearer or table. Revocation therefore
invalidates a later call even when the module retains every descriptive ID.

## Trust and inspection

The Wasmi engine, this profile's import closure, the capability table, and the
selected Base provider remain trusted native code. A confined artifact is not
trusted merely because it has a `.wasm` suffix: its digest and import inventory
must have produced a `PreparedArtifact`. The inspection class is one of:

| Class | Meaning shown by Patchbay/Observatory consumers |
| --- | --- |
| `ConfinedThirdParty` | Artifact executes behind this checked no-WASI boundary. |
| `TrustedNative` | Native Host/Base implementation is deliberately trusted. |
| `LegacyCooperativeNative` | Native code shares process privilege; semantic checks are not hostile-code confinement. |

These classes are not interchangeable security badges. Browser JavaScript is
outside this proof and may independently possess browser authority. The proof
does not claim native-code, kernel, DMA, side-channel, wall-clock deadline, or
physical isolation.

## Proof and stop line

Run `cargo xtask check confined-gear`. The suite executes the production
engine/import provider with pure and effectful modules and hostile fixtures for
forbidden and undeclared imports, wrong digest, growable memory, fuel
exhaustion, Host-call flooding, forged slots, sibling resources, revoked
capabilities, and malformed guest ranges. Forms remain semantic documents and
contain no WebAssembly, runtime, ABI, import, digest, or capability-slot facts.

There is no WASI preopen, general Host adapter, Base registry access, network or
shell convenience import, or automatic retry. Engine fuel exhaustion, import
refusal, malformed memory, capability refusal, Base refusal, and execution
failure remain distinct machine-readable dispositions.
