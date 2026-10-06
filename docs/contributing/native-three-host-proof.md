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

Open `walkthrough.html` in the private evidence directory to review the
actions in order with the actual screenshots, terminal session, and produced
speech clips. It copies the exact Handbook build's shared theme and navigation;
the report hashes that page and both style assets. This development artifact
does not satisfy the full eight-chapter publication gate.

## Start from zero Body without a screen

For a correlated Birth chapter, install the exact Linux release into a **fresh**
`OWNER_STATE` using `conduit host service install ... --no-start`. Do not run
`body own`; the producer starts the installed service and drives its real
`conduit body birth --screen-free` Crèche. It reads the available controls,
edits the name, selects the reviewed Clock Plot, reviews the current Face, and
explicitly activates Birth. The resulting Body is the one provisioned for QMP
and joined by Chromium. Keep the TLS key and the entire new output directory
private: it contains a live invitation and provisioned ISO.
Selected speech first gives command help, the focused Crèche orientation, and
the first available control. The person can request the complete Face with
`read all` when ready. This
producer does request it, including the content beyond the graphical viewport.
The current fixed-storage ConduitOS TLS client offers ECDSA P-256/P-384 and
Ed25519 signatures, not RSA. Use a private P-256 route certificate whose SAN
includes the guest route IP (`10.0.2.42`) and the owner-forward address. An
RSA certificate can provision successfully but the guest cannot complete its
TLS handshake.

```sh
cargo xtask make conduitos screen-free-three-host-proof \
  --owner INSTALLED_CONDUIT --owner-state OWNER_STATE \
  --handbook target/three-host-handbook \
  --build target/conduitos/live/x86_64-pc \
  --host-profile targets/conduitos/profiles/conduitos-native.host.conduit \
  --route-tls-cert PRIVATE_CERT.pem --route-tls-key PRIVATE_KEY.pem \
  --owner-forward OWNER_PRIVATE_IP:37471 \
  --route-url wss://10.0.2.42:4433/conduit \
  --output-dir PRIVATE_NEW_JOURNEY_DIR \
  --playwright proof/browser/node_modules/playwright/index.mjs \
  --body-name 'One Body Clock'
```

`three-host/report.json` binds the retained pre-Birth installation and live
zero-Body service status, Birth input, and actual transcript hashes to the
same Body, owner Host/Boot, release source, and subsequent three-Host proof.
The run ID also binds that owner Host and Boot, so separate captures remain
distinguishable even when identical Birth inputs yield the same Body ID.
`three-host/walkthrough.html` opens with the Birth action and links the
pre-Birth receipt. Without a selected speaker, the screen-free
client emits text readout and this run does not prove audio. To select real
device playback, first inspect `conduit body speech-options --json`, then add
the exact `--speaker-card`, `--speaker-device`, `--speech-executable`,
`--speech-data`, and `--speech-engine` options. The selected device's playback
receipts are in the Birth transcript; this remains distinct from attended
human listening. Pass the exact regular `engine` path reported by
`speech-options`, not a versioned-library symlink; provider discovery rejects
symlinks to keep the bound bytes unambiguous. The producer waits for the next
screen-free prompt before sending each selected-speaker command, so a queued
command does not interrupt a full-Face reading. Current selected playback uses a
second StdHost sharing the
owner's Host and Boot identities; this run does not establish speech realized
by the owner instance. The three speech provider options may also be used without a
speaker selection to retain the same-run direct speech artifact after the
three-Host actions; that does not turn the Birth readout into audio.

When the Face changes during a selected-speaker reading, the client cancels
the stale turn between speech Plays. The capture driver can request the current
Face again, at most four times, and retains every command it actually sent.
The active clock chapter uses concise orientation instead of an automatic
full-Face reading that would outlast its 60-second admitted Play. A cancelled
last turn or a Face that keeps changing fails this proof; neither is a
completed spoken Show.

After the live three-Host actions and speech captures, the same producer
reenters the retained owner with `conduit body screen-free`. The three-Host
producer has already retired the clock Play, so the Body is Lulled. It reads
the current Face on request, focuses and activates the available Start clock
action, receives concise current-Face orientation, then focuses and activates
Stop clock before the bounded Play ends. It reads the resulting Lulled Face in
full. The owner retires
that Play and returns to Lulled. `three-host/report.json` binds both inputs
and transcripts to the original Body, owner Host/Boot, run, source Faces,
acknowledged Shows, semantic actions, and resulting Faces. If a speaker was
selected, the readings must have completed playback receipts; otherwise this
is text readout only. This proves screen-free control of the resident clock,
not a complete screen-free traversal of all three-Host journey chapters or
human listening.

For a same-run finite model explanation, add `--model ALREADY_LOCAL_MODEL` with
the three speech provider options. The optional `--ollama-endpoint` defaults to
`http://127.0.0.1:11434`; `--admitted-memory-mib` defaults to 2048. The
producer opens its own loopback forwarding route to the selected, already-local
Ollama service, calls the existing `one-body-spoken-chapter --mode llm-assisted`
while the QMP guest and Chromium remain live, and checks the original provider
output, accepted finite wording, Face/Show, source/run/Body/Host/Boot identities,
and real WAV. It then closes only that forwarding route and retains the next
request's connection refusal and absence of audio. It then opens a fresh
forwarding route to the same service and requires another live model Play,
validated spoken Show, and WAV from the unchanged owner Face. This demonstrates
loss and restoration of the **configured model route**, not shutdown of Ollama,
Host availability withdrawal, owner-sealed wardrobe replacement, speaker
playback, or human hearing. The private walkthrough has a separate chapter
with both produced explanations and expandable original output, validation,
refusal, and restoration receipts. No fixture or failed model request can stand
in for a successful explanation.
