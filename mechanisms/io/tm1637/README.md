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
loop. No GPIO provider, capability grant, Host Call binding or physical proof is
claimed yet. Integration must run beneath the generic admitted Host boundary;
provider operations and the whole finite transaction need admitted time bounds.

Run `cargo xtask prove environment display-wire` for deterministic wire traces,
NACK locations, cancellation and provider-failure cleanup. These are simulated
electrical traces, not measurements of the physical device.

For the proposed Pico connection, CLK uses GP3 and DIO uses GP4 through a
bidirectional open-drain-compatible 3.3 V / 5 V shifter, with common ground.
The display uses USB VBUS 5 V. No GPIO is assigned for shifter enable; GP5 remains
free. The replacement shifter and actual dot/colon aliases await identification.
