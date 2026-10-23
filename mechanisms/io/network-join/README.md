# Selected native network join

`cargo xtask prove environment network-owner` checks the portable Host Call
owner with CPU providers. `cargo xtask prove environment native-construction`
also binds and invokes it through the native assignment generated from the
actual network Kind. Neither proof initializes a physical radio or installs a Body.

Root supplies real provider ownership, a current opaque capability, an exact
Plan/Play binding, one reserved attachment generation and independently admitted
SDK memory, work and deadlines. The owner reserves its own recurring storage
before Play. An SDK allocator requires a separately admitted finite native pool;
CPU zero-allocation tests do not prove physical SDK memory bounds. The owner checks the existing
`authority/network-configurator` subject through the shared rich or constrained
binder. It grants no authority from serialized assignment metadata.

One setup Play admits one join attempt. Invalid framing, wrong node/call/request,
revoked possession and replay are refused before provider effects. The provider
borrows credential bytes from the kernel input; this owner neither copies nor
logs them. Kernel input and SDK secret lifetime remain their respective owners'
responsibility. A successful provider record must match current Host, Boot,
resource pool and the reserved attachment generation. Provider failure or a
mismatched record returns no successful attachment.

Cancellation drops the provider future before revoking its unfinished lease.
An explicit cleanup obligation remains after invocation. Failure or cancellation
requires bounded restoration. After success, Root may retain the actual owner and
station attachment beyond the completed configuration Play; retirement or link
loss then requires cleanup. The flag is an ownership obligation, not an automatic
request to leave a successfully joined network. Failure or cancellation of cleanup
cannot clear that obligation. Cleanup readiness never
mints a new attachment. The composition root still needs to connect the actual
ESP32 adapter, initialize/project offers, issue capabilities and drive the
credential and attachment Host Calls under the ordinary Body lifecycle.

`installation::ReadyNetworkJoinBase` retains the real initialized provider while
Root advertises its exact Base, implementation and station resource. Its unsafe
constructor requires independent native initialization and memory/work admission;
a descriptor or a successful probe cannot establish readiness. Projection checks
the current Host/Boot and refuses duplicates without mutating the advertisement.
Root must retain the owner and withdraw availability before disposing of it.

Binding consumes this owner. Rich Plans must retain its actual artifact, Base,
profile and resource generation. Constrained assignments are checked against
native identity supplied by the retained owner rather than by the incoming bytes.
The common opaque-capability checks still apply. These are CPU conformance proofs;
the firmware composition root does not yet install or advertise this network Base.

`NetworkControlBack` supplies the finite credentials source and attachment-sign
sink around the ordinary join Host Call Back. All three use the production
scheduler's request/completion path; the source forwards the exact completed
value reference. Private state advances only after transactional I/O commit in
the new endpoints. No radio callback, credential policy or scheduler is hidden
in these Backs. Numeric-kernel fixtures verify one request per operation, source
closure, exact failure forwarding, and no join after missing/cancelled credentials.
They do not establish a checked Plot installation or physical network effects.
