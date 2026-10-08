# Prove one Body on Linux, ConduitOS, and Chromium

This retained `clock-demo` is an **interval ticker and lifecycle specimen**: it
changes how often ticks occur; it does not tell wall-clock time. The Todo Body
in [#5201](https://github.com/dancxjo/conduit/issues/5201) is the human-facing
application journey. The older `One Body Clock` name below identifies the
original captured run and remains in its immutable evidence.

This development proof for [#4807](https://github.com/dancxjo/conduit/issues/4807)
keeps one installed Linux Body owner, one ConduitOS x86_64 QMP guest, and one
pinned Chromium Host live together. The guest changes the checked clock
interval to 500 ms; the browser reads that new Face and changes it back to
1000 ms. The Linux terminal then inspects the same current Face through its
local Mask, changes the interval to 500 ms, and the browser sees that result
while the ConduitOS guest remains live. The owner alone retains the
Body and workload truth. The browser then doffs, wears, and explicitly prefers
its Mask through the owner's controls before requesting a fresh Show. This
proves the three-Host graphical and terminal presentation topology and one
same-Plan browser wardrobe transition. With a selected speaker, it also
retains the installed owner's completed speaker Plays and same-Play WAVs.
It does not prove presentation-host loss, physical hardware, attended hearing,
or the published journey.

The browser joins first. After ConduitOS joins, its screen displays the
owner Face read-only and asks the person to choose Native graphics in the
owner's wardrobe. In the browser, inspect the newly admitted native route,
Wear it, Doff the browser Mask, and Prefer native graphics. Preference alone
does not preempt a currently valid browser Show. Press F5 on the native screen to request a fresh Show;
the owner acknowledges that Show before the native clock action. The proof
drives these same user actions through the browser controls and QMP keyboard.
After the native action, the person wears the browser Mask, doffs the native
Mask, and prefers the browser again before refreshing its Face.
The standby screenshot and owner wardrobe transitions are retained alongside
the action screenshots. An early F5 can be refused; it never changes wardrobe
policy by itself.

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
local Shows, and its semantic action. The resulting `report.json` hashes seven
screenshots, the terminal transcript, and the exact Rust native receipt;
`native/owner-action-proof.json` retains the unrounded Face and Show revisions.
`browser-wardrobe.json` retains the five owner-produced reports, including the
unchanged owner Plan, three explicit policy actions, and fresh acknowledged Show.
Do not copy the invitation or private ISO into the evidence directory or onto
the website. A successful local receipt is not CI, accepted-release, physical,
or public Pages proof.

To retain a direct spoken reading in that same live three-host run, add
`--speech-executable ESPEAK --speech-data ESPEAK_NG_DATA --speech-engine
LIBESPEAK_NG --speech-language-coverage COVERAGE_NATIVE` to the command.
Prepare the explicit provider-bound declaration through
`cargo xtask make host declare-speech-language` as described in the
[runtime speech guide](../proof/runtime-speech.md). Coverage is supplied Host
metadata; provider discovery alone does not establish Language support. Use the installed engine's exact regular file,
not its `.so.1` symlink. The driver calls the existing
`one-body-spoken-chapter` producer after the wardrobe action, while QEMU and
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
The embedded verifier also matches its route name against a DNS SAN or common
name. For this IP route, set the certificate common name to `10.0.2.42` as well
as including the IP SAN. A certificate with only the IP SAN can provision but
will be refused at guest TLS authentication.

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
pre-Birth receipt. The walkthrough bundle retains verified copies of the Birth and later
screen-free clock inputs and transcripts beside the page; the private
invitation, spore, TLS material, and owner state stay outside that bundle.
The browser producer also writes one-time `three-host/observations/*.json`
records when its six screenshots are captured. Each binds the actual triggering
event, current Face and Show, browser Host and Boot, and PNG digest before the
final report is assembled. These observations are development evidence, not
the complete eight-chapter publication receipts.
Without a selected speaker, the screen-free client emits text readout and
this run does not prove audio. To select real device playback, first inspect
`conduit body speech-options --json`, then add
the exact `--speaker-card`, `--speaker-device`, `--speech-executable`,
`--speech-data`, `--speech-engine`, and `--speech-language-coverage` options.
The selected device's playback receipts are in the Birth transcript. The
browser speech action retains the installed owner's completed speaker Play
receipts. Each playable WAV is the PCM delivered to the speaker in that Plan
and Play, verified against the producer's WAV and PCM hashes. A digitally
silent Play remains in the receipt without an audio control. These receipts
remain distinct from attended human listening. Pass the exact regular `engine` path reported by
`speech-options`, not a versioned-library symlink; provider discovery rejects
symlinks to keep the bound bytes unambiguous. The producer waits for the next
screen-free prompt before sending each selected-speaker command, so a queued
command does not interrupt a full-Face reading. The speech provider options
and explicit coverage declaration may also be used without a
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
the speech provider options and explicit coverage declaration. The optional `--ollama-endpoint` defaults to
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

To capture **listener audio** for the model Mask in that same run, install the
fresh owner Host with `--selected-model ALREADY_LOCAL_MODEL
--model-endpoint http://127.0.0.1:11434 --model-memory-mib 3072` as well as
the selected speaker and provider, before starting the service or Birth.
Pass the same model to the proof entrance. The producer then selects the
installed owner's sealed model route, waits for its validated Show and a
separate selected-speaker audio Play, and retains the WAV from that audio
Play's speaker/WAV fan-out. The prior model artifact and the diagnostic
producer's WAVs remain separate, non-listener evidence. This establishes
completed digital delivery to the selected ALSA device, not attended hearing
or model-provider withdrawal from the installed owner's wardrobe.

To exercise loss of the **installed owner's** selected model provider in a new
run, choose an unused loopback port and a private directory before installing
the owner. Select `http://127.0.0.1:PORT` as the fresh owner's exact
`--model-endpoint`. The installer verifies the selected model, so the route
must be alive during installation. Run installation as the first supervised
command, then run the proof as the second supervised command on the same port.
Use a new control socket for each phase; installation uses `--no-start`, so
there is no owner Boot between them:

```sh
mkdir -m 700 PRIVATE_ROUTE_DIR
cargo xtask make conduitos owner-model-route \
  --upstream http://127.0.0.1:11434 \
  --control-socket PRIVATE_ROUTE_DIR/install-control.sock \
  --listen-port PORT -- \
  conduit host service install ... --no-start \
    --selected-model MODEL --model-endpoint http://127.0.0.1:PORT
cargo xtask make conduitos owner-model-route \
  --upstream http://127.0.0.1:11434 \
  --control-socket PRIVATE_ROUTE_DIR/proof-control.sock \
  --listen-port PORT -- \
  cargo xtask make conduitos screen-free-three-host-proof ... \
    --owner-model-route-control PRIVATE_ROUTE_DIR/proof-control.sock \
    --ollama-endpoint http://127.0.0.1:PORT
```

The route reports the selected endpoint after binding it, then launches and
monitors its command. If the route exits unexpectedly, it stops that command
and fails immediately; when the command ends, it closes the route and removes
the control socket. Use the same `PORT` in both invocations and the installed
owner's `--model-endpoint`; also supply `--model-memory-mib` at install. A direct
Ollama endpoint in `installation.json` cannot be substituted later: the owner
reobserves the selected offer at Boot. The producer verifies that the private
control socket names that exact installed endpoint, withdraws only that route,
asks the owner-selected model Mask to start, checks its refusal before a Play
and the absence of a new listener WAV, restores the same endpoint, and captures a new
owner Show and same-Play speaker WAV. The shared Ollama service stays running.
The route and control socket are producer equipment, not authored Plot facts.
An existing installation that selected Ollama directly remains valid for
listener-speech proof but cannot make this owner-provider-loss claim.
