# Raspberry Pi recorder and presenter

The environmental appliance should span one Body with an MCU sensing part and
an optional Raspberry Pi or browser part. The Pi should be able to birth the
Body, use Make to prepare and provision the MCU, retain observations, and offer
a display Mask. Raspberry Pi OS is the first implementation; ConduitOS should
later offer the same semantic roles through its own initialized Backs.

The appliance Plot keeps observation meaning and first-class quantities. The
Host specification records actual storage, display, Line and construction offers.
The Body specification requests the appliance and eligible Masks. Exact Plans
select the actual implementations and finite resources; a board label or an
installed operating system is not a ready offer or storage authority.

## State that must survive a change of Host

Body identity and membership, recording identity and committed observation
sequence, and semantic configuration must remain distinguishable from each
Host's Boot, selected Back, Plan, Play and transport connection. A new Mask
consumes the same Face. A new recorder must resume from an acknowledged durable
prefix with explicit storage authority, rather than infer possession from a
browser connection or replayed status JSON.

Switching between Raspberry Pi OS, ConduitOS and a browser is a lifecycle
operation. Read-only presentation may coexist. Exclusive ownership and recording
handoff require explicit retirement or fencing of the former owner, verification
of the durable prefix, preparation of the new resources, and an admitted new
realization. Unknown outcomes, unavailable stores, pressure and sequence gaps
remain visible. Merely rebooting the Pi into another image proves none of this.

## Acceptance still required

- Identify the exact Pi revision, installed OS, available RAM/storage, display
  and connection to the MCU. Select or add its exact Make profile; do not reuse
  a 64-bit Pi profile for an ARMv6 board.
- On Raspberry Pi OS, birth the real Body, Make and provision the actual MCU,
  admit both parts, and retain the owner across attributed start and release.
- Record actual first-class observations durably while a Pi Mask and browser
  Mask inspect the same Face; prove restart recovery and explicit gap reporting.
- Prepare ConduitOS with actual network, durable-storage and display Backs on
  that board. A boot or emulator result alone does not establish those offers.
- Demonstrate Linux-to-ConduitOS and reverse handoff, plus equivalent browser
  handoff, preserving identity and the durable recording prefix without duplicate
  active ownership. Retain source-bound physical evidence and a real overnight
  recording separately from accelerated logical-time tests.

Current repository inspection found reviewed Raspberry Pi OS packaging for
newer aarch64 profiles and an ARMv6 ConduitOS substrate including Model B+ v1.2.
An unspecified old Model B has not yet been matched to either profile. This
record is the requested construction target, not an implementation claim.
