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

`cargo xtask prove environment display-output` checks the exact owner boundary,
wire traces, canonical Source packet encoding and existing native Face/Mask
interpretation. Tests use explicit CPU pin/authority/selection fixtures. They
cover malformed geometry before effects, substituted Play/ports, stale requests,
NACK, provider loss, cancellation and allocation-free invocation. This does not
install a display Back, admit its full memory/work closure, join an output Line
or prove physical delivery. Assigned firmware binding and actual Root installation
remain separate work.
