# Native Crèche product proof, 25 October 2026

These are local product and QEMU observations. They do not establish merged,
hosted CI, accepted-release, Raspberry Pi 3 B+, or physical deployment proof.
The parent delivery issue is #6458; integration with #6457 remains pending.

## Ordinary native Plot journey

Product source: `d2bce6c09eb9c69e3492eb41d1efecd8a8e65ea7`.
The sealed Linux executable was installed through `conduit host service install`.
An owned parent Body was created through `conduit body own`, retained, and reused
across public installer upgrades. The installed executable entered through
`conduit host service run --state-dir … --creche …`.

The Linux guest used QEMU TCG, QXL, an unmodified Linux 7.0.0-38 kernel and
matching modules, and SDL 2.32.10 KMSDRM. Only the task's private proof state was
exported writable. There was no network or host-home export. QMP key events
passed through native input; no Face, interaction, birth, or outcome was injected.

The journey selected Keyboard canvas, entered and applied the name `garden`,
kept `first-native-pc` as the first Host, selected all declared Hosts, reviewed
the artifact path, prepared both images, saved each, and selected Done.
The ordinary execution completed, its outcome Fore was acknowledged, and the
product and guest exited successfully. Terminal evidence reported no UI or
retention errors. The parent Body was unchanged by installation upgrades.

| Identity | Value |
| --- | --- |
| Parent Body | `7ee2f4ed2808bb88c39813ceeae6d6219c95577854df82adc822f54e00ee900f` |
| Child Body | `1c11b510d4035e6569fe99a7a2d3748568bf94f9af403a1c98521b68e4bda036` |
| Retained operation | `f8099ccf-38a4-4e56-b7ee-8b6202b8cb44` |

Both saved files were independently read and compared with their prepared and
saved receipts. Each contains 29,841,408 bytes and binds the same recorded birth.

| Host entry | Saved artifact SHA-256 |
| --- | --- |
| first-native-pc | `ce27abc46a3c5a5b9632698f8114b6930744a81c2ed374d4d838947be2d58bc3` |
| later-native-pc | `6422342a7714154cba351eb35e408caf44951950f61a6f7a6bc980d2d853d54b` |

## Boot the actual saved destinations

Both exact saved destination files were booted sequentially under native-PC
QEMU. Before boot, verification checked whole-file hashes, the 32 KiB birth
frames and their payload digests, padding, and unchanged base bytes outside the
frames. Hashes were checked again after boot.

The first Host recovered `garden` and its exact selected Source and checked
Plot, then went through BornLulled, Awake, Planned and quiescent-awaiting-input.
Six native Face records included an available Show under the actual Play.
The later Host stayed in World with no Body/Part/Wake/Plan/Play. It displayed
`garden` as a Candidate with birth retained, invitation unavailable, membership
not admitted, owner receipt unverified, and no actions. It did not join or rebirth.

| Raw serial | Complete bytes | SHA-256 |
| --- | ---: | --- |
| First Host | 161,537 | `91c9f64addfbe3c1ab75c85ae82a14376ec35537a8ff33ae057bf7de2caf5e96` |
| Later Host | 28,613 | `84f4b12af17b1fd2716c94f408ac3dbbfcc66f031269ea18c74940d4a1ab80f0` |

Neither serial capture had a trailing partial record. Each guest was terminated
and reaped after observing its required state; this is not a shutdown proof.

## First-screen correction

Source `3b4a1f531bf81ae7ae3851f79eadb1ee9326b61e` completes the initial KMSDRM
swap before issuing display readiness. Previously, the first content frame
could remain invisible until an input-triggered repaint. The corrected sealed
executable is 64,622,888 bytes, SHA-256
`c91072159378b5562f94273e0f187347e91a7c0a54e6cb317aa694d2f9c494a9`.
A separate actual installed run reused the parent Body. An observer sent only
QMP capabilities and screen-capture commands, with zero keyboard/pointer events.
Visual inspection confirmed the initial Crèche view appeared without input.
The short finite encounter then cancelled without requesting child preparation;
that product exit is not counted as a completed manufacture journey. The guest
exited normally. Its PPM SHA-256 is
`b49f273771d32d60653903601989eb92926c345771d46f998d58c4a7e72f57ce`.

## Validation and evidence custody

The generation-7 ordinary Mask regression executed and passed. All 33 native
Mask tests passed, as did strict compositor-feature library/test Clippy.
Strict all-target Linux display Clippy passed for the first-screen correction.
The SDL error guard also refused an actual modeset failure that SDL logged
without populating `SDL_GetError`; a normal void presentation return alone is
not treated as success.

Raw captures, launch commands, source/locks, manifests, birth, receipts, terminal
and canonical evidence remain task-local in `native-kms-guest-20261025` and
`native-creche-saved-boot-20261025`. They are not hosted artifacts in this commit.
The base native-PC Make image came from source
`fda3da3b83dd49d773a62c6dbc9d6a4ae17a203b`; later Linux product changes do not
change that image's source provenance.

Remaining proof includes a native UI source-build journey, Pi 3 B+ SD/BSP and
physical deployment, actual later-Host join, and shared integration/release
acceptance. The reviewed-artifact journey does not substitute for those gates.

## Shared integration observation

Candidate run [40209048119](https://github.com/dancxjo/conduit/actions/runs/40209048119),
for PR #6474 head `50790b05844ad4c71edee846f20bdf5eab9f16d1`, failed its
x86_64 product journey on 25 October. The job recorded checkout
`c10ffc39a491757f290e23bce474303dcb2282ab`; this is not this feature branch's
local product proof. The verifier reported `creche-choice-timeout` waiting for
`creche-three-selected`. Retained guest serial gives the earlier cause:
`CONDUIT_CRECHE_ENTRY_REFUSAL instance=3 sequence=70 request=commit
result=Fault(WorkExhausted)`, with `admitted=64 rtc=64`, followed by interaction
termination and `front-door-action-refused`. Increasing the observation timeout
would not repair that refusal.

The exact [diagnostic artifact](https://github.com/dancxjo/conduit/actions/runs/40209048119/artifacts/12605459815)
was downloaded and inspected, including serial, registers and stack. It remains
negative integration evidence, separate from the successful local installed
Linux journey. Shared admission/provider changes require coordination with their
current owner. PR #6457 remained open at
`aa9a79edefbd0e191bc2212d3877a3d8c84ae56a` when this observation was made.
