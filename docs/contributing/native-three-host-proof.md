# Prove one Body on Linux, ConduitOS, and Chromium

This development proof for [#4807](https://github.com/dancxjo/conduit/issues/4807)
keeps one installed Linux Body owner, one ConduitOS x86_64 QMP guest, and one
pinned Chromium Host live together. The guest changes the checked clock
interval to 500 ms; the browser reads that new Face and changes it back to
1000 ms. The Linux terminal then inspects the same current Face through its
local Mask, changes the interval to 500 ms, and the browser sees that result
while the ConduitOS guest remains live. The owner alone retains the
Body and workload truth. This proves the three-Host graphical and terminal
presentation topology, not Mask preference, speech, physical hardware, or the
published journey.

Start from a clean checkout at one commit. Check tools with
`cargo xtask doctor browser` and the ConduitOS prerequisites in
[the native spore guide](native-spore-provision.md). The browser proof needs the
project's pinned Playwright 1.62.0 Chromium installation, one worker, and no
retries. The QMP route needs QEMU, `nc`, a private IPv4 address on the Linux
owner, and a TLS certificate/key pair whose leaf is valid for the guest-visible
`10.0.2.42` address. Keep the key, invitation, and provisioned ISO outside the
repository in mode-0700 directories. The certificate must still be valid during
the run.

Build all three artifacts from that exact checkout:

```sh
cargo xtask make host release --platform linux --output target/three-host-release
cargo xtask make conduitos live x86_64
cargo xtask make body static \
  --application targets/browser/handbook/handbook.application.template.json \
  --handbook --output target/three-host-handbook
```

Install `hosted-linux-x86_64.json` from that release into a new private
`OWNER_STATE` using `conduit host service install ... --no-start`. Read the
installed `product_executable` from its `installation.json`; use that exact
binary as `INSTALLED_CONDUIT`. Birth the Body with
`INSTALLED_CONDUIT body own plots/clock/main.conduit --state-dir OWNER_STATE`,
then run `INSTALLED_CONDUIT host service run --state-dir OWNER_STATE` and keep
that service running. [The owner guide](shared-body-owner.md) explains the
installation lock and fresh Boot.

Issue a fresh routed invitation while the service runs. Keep this command
running until the proof finishes; `OWNER_PRIVATE_IP` must be reachable from the
host's QEMU guest-forward helper:

```sh
INSTALLED_CONDUIT body invite --state-dir OWNER_STATE --ttl-seconds 600 \
  --route-bind OWNER_PRIVATE_IP:37471 \
  --route-url wss://10.0.2.42:4433/conduit \
  --route-tls-cert PRIVATE_CERT.pem --route-tls-key PRIVATE_KEY.pem \
  --authorize-route > PRIVATE_INVITATION.json
```

Author the checked `.body.conduit` shown in the
[native spore guide](native-spore-provision.md), using the exact Body and
invitation IDs from that private document and the reviewed native Host
profile. Provision with `cargo xtask make body provision-conduitos`, passing
`--route-tls-cert PRIVATE_CERT.pem` and the exact
`target/conduitos/live/x86_64-pc` build. Keep the resulting ISO private.

Run the correlated proof with an output path that does not exist yet:

```sh
cargo xtask make conduitos live-three-host-proof \
  --owner INSTALLED_CONDUIT --owner-state OWNER_STATE \
  --handbook target/three-host-handbook --spore PRIVATE_SPORE.iso \
  --candidate-id candidate/body-owner-route \
  --owner-forward OWNER_PRIVATE_IP:37471 \
  --output-dir PRIVATE_NEW_EVIDENCE_DIR \
  --playwright proof/browser/node_modules/playwright/index.mjs
```

The producer checks that the installed owner, BrowserBundle, and QMP image
have the same source commit. It admits the browser before sealing the guest's
Face: a later membership change would correctly make an already shown native
Face stale. It requires three distinct current Host/Boot pairs before either
user action. QMP keyboard input, browser controls, both owner responses, and
the browser's refreshed Face are real product paths. The terminal transcript
comes from `conduit body terminal` and includes the current Face, acknowledged
local Shows, and its semantic action. The resulting `report.json` hashes six
screenshots,
the terminal transcript, and the exact Rust native receipt;
`native/owner-action-proof.json` retains the unrounded Face and Show revisions.
Do not copy the invitation or private ISO into the evidence directory or onto
the website. A successful local receipt is not CI, accepted-release, physical,
or public Pages proof.

To retain a direct spoken reading in that same live three-host run, add
`--speech-executable ESPEAK --speech-data ESPEAK_NG_DATA --speech-engine
LIBESPEAK_NG` to the command. Use the installed engine's exact regular file,
not its `.so.1` symlink. The driver calls the existing
`one-body-spoken-chapter` producer after the terminal action, while QEMU and
Chromium remain live. It assigns the speech action's run ID before synthesis
and retains the complete producer manifest, receipt, transcript, and every
streamed WAV batch under `speech-direct/`. The combined report checks Body,
source, owner Host/Boot, Face, batch identities, and WAV hashes. This is
produced audio from the current owner Face. The speech Plot does not yet have
an owner-sealed spoken Mask Show; the receipt explicitly records no speaker
playback or human listening.
