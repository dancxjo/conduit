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

The selected physical target is a Raspberry Pi 3 Model B+ with an HDMI screen.
The initial OS preference is Raspberry Pi OS 64-bit, subject to inspection of the
installed image. Its exact Make/Host profile must be added or verified; profiles
for a Pi 4 or an ARMv6 board are not evidence for this board. A local HDMI Mask
and a remote browser Mask should consume the same semantic Face.

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
The selected Pi 3B+ has not yet been matched to a reviewed exact profile. This
record is the requested construction target, not an implementation claim.

## MCU-served browser Host and alternate Lines

The MCU's Body page must bootstrap a real browser Host in the same Body, using
the existing browser membership runtime, Host identity and fresh Boot identity.
The browser offers its actually initialized Face, Line and optional durable-store
Backs. Body admission follows the Body's policy; visiting or downloading WASM is
not itself membership, storage possession or owner authority. An admitted browser
Mask consumes the appliance Face rather than maintaining a second appliance.

The bootstrap may load an exact versioned WASM distribution from a served URL.
That distribution must remain reachable in the selected network mode. In an
isolated AP without Internet, a remote URL alone is insufficient: retain a local
asset source or a proved cache path, including first-visit behavior. Keep the
runtime asset identity and loader origin distinct from the Body identity and
selected transport; test resource loading and storage-origin behavior explicitly.

Departure uses the existing presence/session machinery: a graceful departure is
useful evidence, but abrupt tab loss, navigation and suspended pages also need
bounded disconnect or lease expiry. Durable membership, unavailable presence,
retained recording bytes and authority retirement are different facts. Returning
must follow ordinary admission/return rules and fence stale Boots.

Bluetooth is also requested as an alternate Line. It must be independently
initialized, offered and selected, with exact peers, finite queues, framing,
authority, correlation, failure and planned fallback behavior. It does not grant
unplanned connectivity, duplicate request replay or automatic retries. Prove the
MCU-to-Pi and MCU-to-browser realizations separately; the browser's actual
transport support and permissions must be observed. Wi-Fi loss and Bluetooth
loss must preserve the Body identity and report gaps or unknown outcomes.

Existing entry points to reuse are
`targets/browser/host/assets/browser-host-membership.mjs`,
`targets/browser/profiles/environment-recorder.host.conduit` and
`architecture/body/src/presence.rs`. This appliance has not yet wired or physically
proved those membership and alternate-Line paths.
