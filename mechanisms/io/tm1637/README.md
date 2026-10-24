# TM1637 wire transport

This portable, allocation-free transport consumes borrowed packets produced by
`plots/device-protocols/tm1637.conduit`. The seven-segment Mask and its selected
annunciator profile own formatting and register mapping. This crate owns only
START, STOP, least-significant-bit-first transmission and per-byte ACK sampling.

`OpenDrain` releases a line for a logical high and drives it low for a logical
low. The selected provider must not drive either signal high. It owns bounded
GPIO and delay effects, exact resource authority and any electrical translation.
The fixed half-period is at least 5 microseconds. At most three packets of seven
bytes are accepted, with no retry. Cancellation is checked before each packet
and byte. A NACK produces a STOP; provider loss attempts both line releases.
Cleanup cannot guarantee an electrical state after a failed GPIO provider.

This is a driver component, not an installed Host Back or a second execution
loop. The optional Host Call binding below supplies exact planned possession, but no
initialized GPIO provider, capability grant or physical installation. Integration
must run beneath the generic admitted Host boundary;
provider operations and the whole finite transaction need admitted time bounds.

Run `cargo xtask prove environment display-wire` for deterministic wire traces,
NACK locations, cancellation and provider-failure cleanup. These are simulated
electrical traces, not measurements of the physical device.

The present ESP32 bench uses GPIO18 for CLK, GPIO19 for DIO and a common
3.3 V/GND supply, without the earlier level shifter. The checked bench profile
has only the central colon connected (register 1, bit 7); bottom decimal points
failed the attended all-segment/raw-bit test. Electrical facts belong to the
selected hardware profile and provider, not to the semantic observation.

With `host-call`, `WriteContract` consumes the existing authored `Tm1637Packets`
Type. `WriteHostCall` binds an independently owned provider to an exact sealed
Plan, ordinary lowering, current Play, Base capability and authority. Canonical
packet validation, finite lengths and zero padding are checked before effects.
The synchronous provider must terminate each edge and half-period within its
independently admitted work bound. Each request consumes one operation and emits
no value; a stale request cannot replay it. Electrical failure or cancellation
attempts both releases, completes the operation and revokes further use. There
are no automatic retries. Revoked idle retirement returns the original provider
and attachment; fresh authority is required to bind them again.

`cargo xtask check tm1637-output` checks the exact owner boundary, wire traces
and canonical Source packet encoding. Environmental Face/Mask interpretation
remains a separate appliance proof. Tests use explicit CPU pin/authority/selection fixtures. They
cover malformed geometry before effects, substituted Play/ports, stale requests,
NACK, provider loss, cancellation and allocation-free invocation. This does not
install a display Back, admit its full memory/work closure, join an output Line
or prove physical delivery.

`ReadyWriteBase` retains the actual provider before projecting its Ready Base,
resource and zero-result offer through the existing registry. A foreign Boot or
colliding advertisement refuses without mutation or electrical work. Native
identity remains independent of assignment bytes. Both contiguous and borrowed
assigned projections use the ordinary exact binder. Claim/identity, assignment,
capability bounds and packet codec preparation all finish before
`bind_assigned_retained` takes Root's slot; refusal preserves the provider.
The assigned tests drop the assignment storage before allocation-free invocation,
then revoke and return that same provider through idle retirement. These are CPU
fixtures, not actual initialized ESP32 pins, Root installation or complete native
memory/work admission. The running firmware has not adopted this component yet.

`WriteBack` is the finite sink in the production kernel. It consumes packet input,
requests the selected zero-result Host Call and waits for that exact completion.
Its request phase advances only when the ordinary Step commits. A cancelled or
refused transaction cannot advance a private request counter or manufacture an
acknowledgement. The prepared operation ceiling refuses excess input before
another electrical request, and normal input closure drains after pending work.

The kernel proof sends two Source-encoded packet frames through the real fixed
scheduler, services each original request with `WriteHostCall` and completes the
same request. It checks NACK, cancellation, operation exhaustion, late completion
after kernel cancellation and zero heap allocations across execution. Numeric
routing/admission, native pin ownership and authority remain explicit fixtures;
this is not an installed MCU Mask Play or physical-display proof.

The selected packet decoder borrows the independently checked, linked packet
schema. Preparation compares the complete contract Type with those bytes;
execution still checks the full canonical envelope, every primitive leaf, packet
geometry and zero padding before GPIO effects. It retains no decoded schema heap.
`cargo xtask check tm1637-output` includes refusal parity against the previous
owned validator for every truncation, malformed leaf, foreign prefix and repeated
use, alongside the selected-owner and electrical tests. The measured 794-byte
reduction is CPU64 validator heap, not complete MCU or appliance admission.
