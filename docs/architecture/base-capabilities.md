# Base capability possession

Status: universal possession and revocation contract; mechanism-specific
confinement is proved separately. Owner: #3072 under epic #3069.

## Law

An `AuthorityGrantId`, capability ID, Host/Boot identity, Plan field, Base or
resource identity is descriptive. Knowing or copying it grants no permission.
An effect provider accepts only an opaque capability issued from trusted current
authority after Plan and Play admission.

`conduit-core::BaseCapabilityTable` supplies the shared contract. Its issuer
owns private key material and a bounded table. `BaseCapabilityHandle` has no
public constructor, serialization, or bearer-revealing debug form. Patchbay,
Signs, and logs may expose `CapabilityPossessionId`, exact scope, lifecycle, and
accounting, but never bearer bytes or issuer key material.

## Exact scope

Every issued capability binds:

```text
Host / Boot
Base provider instance / generation
Plan / active Play
authority grant / authority contract / selected capability
selected implementation
operation contract / subject
resource pool / resource generation
Base-owned parameter-envelope identity
maximum parameter bytes / result bytes / work
maximum in-flight operations / total operations
```

The trusted authority ceiling contains the same current Base, operation,
subject, resource, and envelope facts plus upper bounds. Issuance rejects any
requested scope which differs or broadens a bound. The planner can therefore
narrow authority, never create or enlarge it.

The provider validates the bearer and every claimed current field at operation
time, then admits one finite lease. Completion is correlated to that lease and
its revocation generation. Revocation clears outstanding leases; a late
completion is stale evidence, not success. Exhaustion, pressure, wrong scope,
wrong envelope, revocation, unknown capability, unknown lease, and stale
completion remain distinct.

Play cancellation/completion, Plan replacement, authority revocation, and
resource-generation replacement revoke matching entries. A Boot or Base
provider replacement creates a fresh table with a fresh issuer key; old bearer
values are unknown. Semantic State continuity never transfers possession.

## Mechanism mappings

The shared scope and lifecycle stay the same while possession mechanisms differ:

| Boundary | Mapping | Proof limit |
|---|---|---|
| Local trusted provider | Opaque handle backed by the private bounded table; an IPC slot may carry it without serialization | Establishes the reusable provider contract, not process isolation or denial of direct syscalls |
| WASM | One restricted import closure captures a private table handle; module data supplies only the operation claim | Requires actual import inventory, memory/work bounds, and adversarial module execution |
| Hosted isolated process | A restricted IPC endpoint/slot or OS-protected transferred handle maps to one table entry | Requires real OS sandboxing which prevents bypass of the provider channel |
| Remote provider | Authenticated delegation binds the same scope plus peer identity and replay state | Requires cryptographic delegation and receiving-provider validation; TLS alone is insufficient |
| ConduitOS | An opaque index resolves only inside a kernel-owned table for the caller's protection domain | Requires actual privilege/memory/device isolation; a shared address space is not proof |

Domain-specific Bases own envelope semantics. A file Base can bind read-only
access to one already-resolved file/directory resource and byte limit; a network
Base can bind one protocol/endpoint/redirect policy and request budget; a ROS
Base can bind one direction/topic/type/rate or actuator envelope. Envelope
identity never becomes a wildcard or a second authority system.

## Non-claims

This contract makes copied public IDs insufficient at a compliant provider
boundary. It does not itself prove that hostile native code cannot bypass that
provider using ambient OS authority. Process, WASM, remote, kernel, and physical
enforcement each require their own positive and adversarial proof against the
actual last trusted effect seam.
