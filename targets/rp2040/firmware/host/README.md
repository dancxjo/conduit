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
