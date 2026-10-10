# Thermostat

A thermostat encounter beside Todo in the reviewed plot shelf. Its settings and
observations have exact typed Forms; `main.conduit` composes the portable
`thermostat/combine` Kind. The browser projects the thermostat's semantic Face
contribution and sends offered actions back to a hosted production kernel.
JavaScript owns presentation and carries no thermostat transition rules.

Open the app from the repository:

```sh
cargo xtask prove journey thermostat
```

Open the printed loopback address in a browser. Choose another port if needed:

```sh
cargo xtask prove journey thermostat --port 8766
```

The responsive interface has 0.5°C target adjustments, Off/Heat/Cool/Auto modes,
Auto/On fan requests, and Comfort/Eco/Sleep presets. Selected choices, unavailable
controls, current temperature, and requested control status come from the same
Face contribution. At 10°C and 30°C the corresponding adjustment is unavailable.
Actions against an old Face revision refuse without changing state.

Presets are exact application policy: Comfort/Eco/Sleep request 21/18/19°C in
Off, Heat, and Auto modes, or 24/26/25°C in Cool mode. Changing mode updates a
named preset; a manually adjusted Custom target remains unchanged. Eco relaxes
heating or cooling demand; it makes no claim of measured energy savings.

This is a session-local repository encounter. Reloading the browser reads the
same running process's state; stopping the process discards settings. It does
not borrow Todo's checkpoint authority or start an installed Body. The current
browser encounter consumes a Face fragment, without claiming an owner-sealed
Mask Plot route or an acknowledged canonical Show. Those lifecycle and durable
storage integrations remain separate work.

No physical equipment or sensor is connected. The initial observation is absent,
so the interface says **Sensor unavailable**. Typed `Observe` commands can supply
or withdraw an observation through the same Kind, but the app exposes no control
that fabricates a reading. The pure demand projection distinguishes Off, missing
observation, Heating requested, Cooling requested, and the ±0.5°C target deadband;
it never asserts that equipment actually ran. This is a control encounter,
not a hardware controller.

## Contracts and proof

State is one canonical 13-byte Form: version, revision, target in signed tenths
of a degree Celsius, mode, fan, preset, and optional observed temperature. Each
command is exactly three bytes. Targets admit only 10–30°C at 0.5°C resolution;
observations admit −50–80°C at 0.1°C resolution. Unknown discriminants, trailing
bytes, noncanonical absent observations, inconsistent presets, invalid ranges,
and exhausted revisions refuse distinctly. Applying an unchanged setting is
idempotent. Preparation admits bounded storage; the transition Back uses fixed
arrays and does not allocate during a Step.

Every accepted browser action prepares the checked authored Plot, selects its
exact installed Back, and executes it through `KernelCompositeHost`. The adapter
commits only a decoded output after normal kernel completion. Each invocation
retains a distinct parent-bound child Play identity. Face revisions advance even
for idempotent actions, so a completed interaction cannot reuse old controls.
HTTP requests are bounded and served only on the selected loopback origin.

Run the focused semantic, kernel, and browser proof:

```sh
cargo xtask prove journey thermostat --verify
cargo xtask check plots check
```

Browser proof requires the repository's pinned Playwright installation and
Chromium, as described in [browser proof](../../proof/browser/README.md).
It uses one worker and zero retries. It exercises every control family,
both target limits, same-session reload, stale-action refusal, and mobile layout.
The inventory declares the authored deterministic kernel oracle separately from
browser Host execution; the encounter currently runs its kernel on the std host.
