# Control one thermostat across Masks

This is the intended companion to [Keep one Todo list across Masks](todo.md):
start Living room, change its requested temperature in the browser, join the
same running thermostat from a terminal, choose Heat and Eco there, and see
18 °C in the browser. Lull and start again to observe the same Body with a new
Play and the authored 21 °C initial state.

## Keep the thermostat encounter focused

The primary thermostat view should foreground its requested temperature, mode,
fan and preset, with sensor availability and equipment confirmation alongside
those settings. The generic Body Face currently contributes Lull, Body overview,
Plots library and inspection controls to the same rendered surface. Those are
Body-management actions, not thermostat settings.

Keep Body management in a separate Body menu or inspection surface. In this
journey, Off is the thermostat's requested climate-control mode; Lull is an
explicit later Body-management chapter that stops the Play. Do not make Lull a
peer of Heat, Cool or Off, or suggest that it preserves session settings. The
current captures still show the generic renderer; a focused presentation and
its accessibility verification remain part of the unfinished journey work.

## Current capture blocker

There is no complete captured thermostat journey checked in or published by
this change. The live capture reached the browser controls and finite bounds,
then the installed Owner refused terminal attachment with
`terminal-route-requires-lulled-body`. The generic terminal route currently
requires a lulled Body for attachment, refresh, wardrobe and retirement. Todo
can remain idle between committed edits; Thermostat owns a continuous scan.

Selected audio also requires an idle installed Owner. The mechanical spoken
reader's deterministic thermostat test does not establish live selected audio
while the scan runs. A speech refusal is a diagnostic chapter, not a successful
spoken encounter. Full Todo parity requires a separately reviewed runtime
change admitting these presentation effects without replacing the thermostat
Play, inventing a second thermostat state, or changing offers beneath its Plan.

Do not restart the thermostat to make a terminal edit look like a same-Play
interaction. Do not substitute the browser-local Handbook thermostat: it is a
separate Body. Preserve the original failed run before attempting another
capture. A new proof attempt needs a fresh private installed Host and a new
output directory.

## Capture entrance

Prepare a native Linux release and packaged Handbook using the supported
entrances in [the Todo setup](todo.md#prepare-the-release-and-handbook), then
install a new private Host with `--no-start`, without Todo checkpoint authority.
Use the installed executable printed in its installation, with a short private
state path (the full `control.sock` pathname must fit Linux's 107-byte limit).
Prepare pinned Chromium using [the browser proof setup](../../proof/browser/README.md).

Once the runtime prerequisite is satisfied, run:

```sh
cargo xtask prove journey thermostat --capture \
  --state-dir /tmp/conduit-thermostat/state \
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
browser/terminal packet deliberately declares audio unavailable; it does not
meet the remaining spoken-Mask acceptance requirement.

## Acceptance still needed

- Admit and acknowledge a terminal Mask without stopping the thermostat scan.
- Read browser-made settings in that terminal, apply Heat/Eco through its typed
  actions, and observe 18 °C in the browser with unchanged Body/Plan/Play.
- Refuse stale browser and terminal Shows without changing requested settings.
- Admit the spoken Mask under an exact presentation Plan and selected provider;
  capture a concise current thermostat reading with correlated output receipts.
  A file artifact and actual speaker delivery remain different proof classes.
- Lull and return to the same Body, showing session reset honestly rather than
  claiming Todo-style checkpoint recovery.
- Capture fresh complete evidence; render and visually verify desktop/mobile
  pages, verify local links, and pass the repository publication gates before
  describing the journey as publicly available.

No sensor or HVAC equipment is connected by this journey. Target temperature,
mode, fan and preset describe requested settings. Sensor availability and
confirmed equipment activity must remain explicit.
