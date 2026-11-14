# Generic RP2040 Host composition

This composition joins through the existing invited admission controller. Its
Host identity is `rp2040/flash/<physical-flash-unique-id>`; changing firmware,
configuration or private Seed does not change that identity. Each boot generates
an independent Boot identity. It publishes no Signal or sampling computation
Backs. Its existing local sensor/display/HTTP tasks remain diagnostics, separate
from membership and from an ordinary sampling Plot/Plan/Play.

Build through the existing repository entrance:

```sh
cargo xtask make host build targets/rp2040/profiles/pico-w-house-green.host.conduit --output target/green-host
```

The public output includes `host.uf2`, `build-manifest.json` and the exact checked
`resolved-image.json` used to bind firmware and private Seed construction.
The public application UF2 is checked to exclude credential and bootstrap pages.
The build requires clean committed Source and verifies it remained unchanged.
No build command flashes a device. Existing build-bound canonical USB BOOTSEL
recovery remains available even when the private Seed is absent or refused.

An optional private Seed input and separate private destination can be supplied
with `--private-seed-input` and `--private-seed-output`. The input contains the
actual physical UID, independently selected current owner FederationPeer,
canonical `PicoSpawnProvision` and optional retained `MembershipCredential`.
The native Seed stores the exact checked configuration, physical Host identity,
owner pin, invitation and retained credential. Its invitation secret is also the
member continuity signing seed, matching Core's committed invitation key. Never
publish the input, native Seed or its containing private directory.

Private Seed output contains only the reserved bootstrap sector, uses create-new
publication with owner-only file access, and must remain outside public output.
No runtime flash writer exists here. A retained credential currently refuses
fresh admission until canonical member-return integration is implemented.
The initial owner pin is scoped to its selected owner Boot, not a durable owner
identity. No cold-return or ordinary sampling-Plan claim is implied.

USB uses existing Conduit bounded u16 big-endian stream framing (4 KiB maximum):
`PICO_SPAWN_REQUEST`, existing advertisement and signed spawn-join request, then
after the actual owner Core commit an attestation metadata frame and the exact
existing portable receipt frame. The device verifies its independently pinned
owner, invitation/member key, signature, full actual Host advertisement and
Body before recording current-Boot membership. Failure leaves local IO running.

The Bosch/HT16K33 drivers, local measurement/Face/display and read-only HTTP
handler are the narrowly reused working implementation from
`52b03dd2b39d4bb4ee5f04d1958d5851929dca24`. The network journal is an exact
extraction of that implementation's v1/v2 custody format, with no format change.
Only read access is instantiated here. The radio adapter reuses the pinned Pico W
CYW43 initialization and fixed onboard wiring; credential bootstrap is read-only.

Source presence and successful compilation do not prove physical membership,
measurement/display continuity, Wi-Fi recovery, retained-flash integrity or a
sampling Plan. Deployment requires the exact reviewed application/private Seed,
actual device inventory and corresponding physical evidence.

## Checked construction checkpoint (2026-11-03)

Ordinary `make host build` completed from clean committed Source
`c8b4d6cef3`, including its exact resolved Host image and manifest binding.
The linked application consumes 863,692 flash bytes and 93,640 static RAM
bytes, including its 49,152-byte heap arena. These are linked extents, not a
measurement of peak execution memory. The UF2 excludes both retained credential
and private bootstrap sectors. See
[`rp2040-ordinary-make-20261103.json`](../../../../proof/evidence/creche/rp2040-ordinary-make-20261103.json).

The next sensor execution path should reuse the existing shared
`plots/device-protocols/bme280-exchange.conduit`, its capture-to-observation
conversion, and finite I2C/clock Host Calls. The parameterized
`environmental-exchange.conduit` combines sensing and HT output through one
selected bus; `bme280-autonomous.conduit` remains an isolated selection example. The diagnostic Rust sensor loop is
not a replacement for that admitted Plot. A Body construction description can
already enumerate Host configurations and Seed outputs; authored Line topology
and complete multi-Host workload construction remain separate integration gaps.
No central all-device project, physical admission or sampling Play is claimed by
this construction checkpoint.

## USB admission carriage

When a private Seed is supplied, Make also emits public `usb-admission.json`:
its exact physical Host/configuration, Spore/image, public invitation claim,
member verifying key and independently selected owner peer. It contains no
invitation secret. After deploying that exact Seed, use the ordinary installed
owner's protected admission service through:

```sh
conduit body admit-usb /dev/serial/by-id/EXACT-RUNTIME-HOST usb-admission.json \
  --state-dir /absolute/owner/state \
  --receipt-output /absolute/private/new-receipt.json --authorize-admission
```

The client checks current owner identity before touching USB, checks the actual
device's proof and construction pins, asks Core to commit ordinary membership,
then obtains and saves the signed owner receipt before USB delivery. Receipt
output is create-new and owner-only. A transport failure after commit does not
undo membership or authorize fresh admission; recover using the retained signed
receipt. An empty output file is not a receipt. A local-control timeout leaves
commit outcome unknown. Successful delivery still needs independent device
acceptance evidence and creates no Plan or Play. The owner pin remains scoped
to the selected owner Boot, and device restart/return remains unfinished.

## Bounded I²C construction (source checkpoint)

The bus initialization now requires explicit `maximum_transaction_polls` in the
Host Source. It must be a nonzero U32; omission and invalid values refuse. The
checked value enters generated firmware configuration and changes construction
identity. The diagnostic sensor/display path consumes the actual configured
controller through `conduit-rp2040-i2c`; it does not create another bus owner or
an available Back. Protocol interpretation is unchanged.

Source `b494ba61b7` links at 863,784 flash-load bytes and 93,536 static RAM bytes,
including the unchanged 49,152-byte heap arena. The unchanged 131,072-byte static
ceiling and 16 KiB minimum-stack assertion pass. These are linked extents, not
execution peaks. This direct compiler control did not produce an ordinary Seed
and has not been deployed. Ordinary Make/Crèche must qualify a new artifact and
Seed before physical installation; the running member remains on its prior Boot.
