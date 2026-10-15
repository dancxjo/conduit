# Control one thermostat across Masks

This is the intended companion to [Keep one Todo list across Masks](todo.md):
start Living room, change its requested temperature in the browser, join the
same running thermostat from a terminal, choose Heat and Eco there, and see
18 °C in the browser. Lull and start again to observe the same Body with a new
Play and the authored 21 °C initial state.

## Keep the thermostat encounter focused

The primary thermostat view should foreground its requested temperature, mode,
fan and preset, with sensor availability and equipment confirmation alongside
those settings. The generic Body Face contributes Lull, Body overview,
Plots library and inspection controls. The browser component groups its Body,
Plot and context subjects behind a collapsed Body menu. Those are
Body-management actions, not thermostat settings.

Keep Body management in a separate Body menu or inspection surface. In this
journey, Off is the thermostat's requested climate-control mode; Lull is an
explicit later Body-management chapter that stops the Play. Do not make Lull a
peer of Heat, Cool or Off, or suggest that it preserves session settings. The
complete capture opens this menu separately from the requested settings.

## Current runtime and capture status

The compact terminal view renders primary typed values from their original
Face contracts. A unique primary Region focuses the application settings;
Inspect retains the complete Face and Body context. The capture requires actual
acknowledged terminal frames showing 21 °C, the browser-made 22.5 °C, and Eco's
18 °C. Canonical state alone is not a substitute for visible terminal readback.

The direct `--owner-show` terminal attaches a provider to the owner's own Host
and requires a lulled Body. A running thermostat instead uses a separately
installed presentation Host. Its real terminal provider is attached before
signed admission, and the owner selects its exact terminal Mask Plan and three
directional Lines. Typed actions return through the existing thermostat worker.

A development live run has verified fresh installed owner and peer processes,
pinned TLS admission, actual terminal write/flush acknowledgement, Heat and Eco,
and unchanged Body/Plan/Play. Closing the terminal leaves the thermostat running.
The same path also passes with an explicitly selected artifact speech provider
attached before admission. This is a same-computer, distinct-Host proof, not a
claim of separate physical computers, equipment control or automatic failover.

While the owner is still lulled, issue its normal routed invitation with an
explicit non-loopback endpoint and selected TLS certificate. Install a fresh
presentation Host with `--no-start`; it must not own or have joined another Body.
Use that peer's installed executable:

```sh
PEER_CONDUIT=/absolute/path/to/peer-installed/conduit-linux-x86_64
"$PEER_CONDUIT" body terminal --state-dir /tmp/thermostat-peer \
  --invitation /absolute/path/to/owner-invitation.json --authorize-join
```

Start the thermostat through its owner, then enter `show` on the peer to refresh
its Face. `actions` lists the current choices; `action thermostat.mode.heat` and
`action thermostat.preset.eco` return typed settings. `evidence` reports the
actual Show and foreground receipt. The finite owner return permits four
actions and four refreshes within sixty seconds. A new encounter needs a fresh
admission; this command does not silently extend a grant or replay a prior Boot.

No complete thermostat journey packet is checked in or published yet. The
producer below now joins a fresh installed presentation peer and captures
browser, terminal and spoken encounters together. An explicit checked
`spoken/control-template` selects the
primary application Region's human text, typed values, actions and temporal
facts from the original Face. It retains requested settings, available actions
and equipment caveats while keeping structural metadata and Body context out
of the reading. Labels are read directly from that same Face.
The full-Face Brief and Expanded selections retain their existing contracts.

With explicitly installed artifact speech equipment, enter `read settings` on
the peer. It requests the checked Control Mask for its exact current Face,
produces the reading through ordinary kernel Plays and WAV acknowledgements,
and returns to the terminal. `evidence` then includes the spoken route, source
Face, ordered frame receipts, provider identity and artifact hashes. Narrating
actions does not admit an interaction Line or authorize speech actions.

A local development run used fresh installed owner and peer Hosts on one
physical machine. Heat and Eco changed the same running thermostat; the spoken
reading completed in 9 artifact frames, and the peer returned to the terminal
with the same Body, Plan and Play. Closing the peer left the scan running.
This proves current owner-correlated artifact generation, not speaker playback
or listening. A speech refusal is not a successful spoken encounter.

The hosted presentation profile admits a 64 KiB route binding and a 128 KiB
control document. One observed reading required 47,568 and 69,002 bytes,
respectively. The native profile retains its 40 KiB binding and 64 KiB document;
physical native frames retain their existing bound. Actual Face and Show values
must still fit the separately admitted 64 KiB hosted Mask Lines. Changing an
encoding profile invalidates its seal rather than conferring new authority.

Do not restart the thermostat to make a terminal edit look like a same-Play
interaction, or substitute the browser-local Handbook thermostat: it is a
separate Body. Each capture attempt needs fresh private installed Hosts and a
new output directory.

## Capture entrance

Prepare a native Linux release and packaged Handbook using the supported
entrances in [the Todo setup](todo.md#prepare-the-release-and-handbook), then
install a new private Host with `--no-start`, without Todo checkpoint authority.
Use the installed executable printed in its installation, with a short private
state path (the full `control.sock` pathname must fit Linux's 107-byte limit).
Prepare pinned Chromium using [the browser proof setup](../../proof/browser/README.md).

Install a second fresh Host from the same release with explicitly selected
artifact speech equipment and `--no-start`. Pass its state directory too:

```sh
cargo xtask prove journey thermostat --capture \
  --state-dir /tmp/conduit-thermostat/state \
  --peer-state-dir /tmp/conduit-thermostat-peer/state \
  --conduit-bin /absolute/path/to/installed/conduit-linux-x86_64 \
  --handbook-package /absolute/path/to/packaged-handbook \
  --output "$PWD/target/thermostat-journey" \
  --pinned-playwright "$PWD/proof/browser/node_modules/@playwright/test/index.mjs"
```

The producer births its own fresh Living room Body, owns its foreground service,
and stops that service on success or failure. It uses one pinned Chromium and
one attempt. Browser and terminal observations must retain the exact Body,
original Plan and Play. A prior browser Show must refuse after terminal edits.
Lull withdraws thermostat actions; the subsequent Play must have a different
identity, retain the Body and restore 21 °C. The driver retains screenshots,
terminal input/output, canonical Faces and exact refusal diagnostics.

Success writes a digest-bound `manifest.json`. Failure retains diagnostics and
must not be copied into the publication evidence directory. The site renderer
admits only a validated packet at `site/evidence/thermostat-journey/`; absent
that packet, it creates neither a thermostat page nor a journey card. The
complete packet includes the exact spoken wording, ordered frame receipts,
original WAV files and a derived concatenation for listening. Publication rejects
dirty capture tools, mixed source versions and partial encounters.

## Publication still needed

The combined development capture has passed browser and terminal changes, both
stale-Show refusals, nine spoken output frames, return to the same running Body,
and Lull followed by a new Play at 21 °C. These development captures use mixed
source versions and are deliberately refused by the publication validator.

Build the installed owner, peer, browser runtime and Handbook from the same
clean committed dev source, then capture a fresh complete packet. Render and
visually verify desktop/mobile pages and local links, and pass the repository
publication gates before describing the journey as publicly available.

No sensor or HVAC equipment is connected by this journey. Target temperature,
mode, fan and preset describe requested settings. Sensor availability and
confirmed equipment activity must remain explicit.
