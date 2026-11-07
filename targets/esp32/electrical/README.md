# ESP32 electrical providers

`conduit-esp32-electrical` supplies owned ESP32 GPIO realizations for the existing
class-neutral pulse-capture and TM1637 mechanisms. It contains no environmental
application, Body identity, protocol decoder, scheduler, or display formatting.

The trusted Host composition selects physical pins and pull-ups. For Blue, the
current verified attachment description is capture GPIO27, TM1637 clock GPIO18,
and data GPIO19. These are construction inputs, not hardcoded provider policy.
An admitted selected-operation owner must bind actual equipment possession before
these providers are used by a Play. The DHT11 request and decoding remain authored
in `plots/device-protocols`; presentation remains the shared Face and Mask path.

Pulse capture accepts only the provider's finite geometry envelope and rechecks
it before effects. It blocks cooperatively for at most 50ms drive plus 50ms
capture according to the local clock, leaves interrupts enabled, allocates
nothing, and releases the line before returning. This synchronous extent must be
admitted by its Host's execution budget; it is not a background polling task.
Interrupt latency can corrupt an observation, which the authored protocol checks
must reject. Timeout exposes only complete octets and remains distinct from a
complete frame. The `host-call` feature implements the existing
`PulseReadProvider` contract; construction alone supplies no authority.

The TM1637 implementation consumes two owned pins, drives only low/release,
selects finite 5–1000us half-period timing, and releases both pins on retirement.
Register content and checked transaction bounds belong to the existing Mask and
TM1637 transaction mechanism. No display blanking or environment formatting is
added here.

These providers are support for a shared Host composition, not themselves a
bootable firmware or proof that Blue has joined or executed a Body Play.
