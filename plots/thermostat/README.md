# Thermostat

A thermostat encounter beside Todo in the reviewed plot shelf. Its settings and
observations have exact typed Forms; `main.conduit` defines a retained scan Plot over the portable
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

Play the canonical Plot through the regular product entrance:

```sh
cargo +stable run -p conduit -- run plots/thermostat/main.conduit
```

Enter one JSON control per line. Targets use signed tenths of a degree Celsius:
`225` means 22.5°C. For example, this finite input completes one Play on EOF:

```sh
printf '%s\n' \
  '{"command":"set-mode","mode":"heat"}' \
  '{"command":"set-target","target":225}' \
  '{"command":"set-fan","fan":"on"}' \
  '{"command":"set-preset","preset":"eco"}' \
  | cargo +stable run -p conduit -- run plots/thermostat/main.conduit \
      --report /tmp/thermostat-run.json \
      --artifacts /tmp/thermostat-artifacts
```

Each resulting state prints as JSON. Modes are `off`, `heat`, `cool`, and `auto`;
fans are `auto` and `on`; presets are `comfort`, `eco`, and `sleep`. The canonical
scan admits at most 256 controls in one Play. Each input line is bounded to 1024
bytes. EOF closes its Fore and completes that Play; invalid input stops it.

The report uses `conduit.thermostat/native-body-run@1` and retains the checked
source Plan, sealed BodyPlan, actual Body Play and terminal Sign identities,
child Sign receipts, and state outputs. The artifact directory must be new; it
contains `source.conduit`, `plan.json`, `body-plan.json`, `play.json`, `sign.json`,
and `report.json` under the corresponding native schemas. The source Plan and
BodyPlan have distinct identities, retained together so the parent Play can be
traced to its actual sealed BodyPlan. CLI placement and Body overrides are
currently unavailable for this exact local scan route.

The responsive interface has 0.5°C target adjustments, Off/Heat/Cool/Auto modes,
Auto/On fan requests, and Comfort/Eco/Sleep presets. Selected choices, unavailable
controls, current temperature, and requested control status come from the same
Face contribution. At 10°C and 30°C the corresponding adjustment is unavailable.
Actions against an old Face revision refuse without changing state.

Presets are exact application policy: Comfort/Eco/Sleep request 21/18/19°C in
Off, Heat, and Auto modes, or 24/26/25°C in Cool mode. Changing mode updates a
named preset; a manually adjusted Custom target remains unchanged. Eco relaxes
heating or cooling demand; it makes no claim of measured energy savings.

The browser starts an ordinary installed std Body: the authored source is
checked and expanded, a Plan is sealed into a BodyPlan, and one live Play receives
typed commands through its Fore. The scan owns the retained accumulator. Reloading
reads that same Play; normal Rust teardown requests ordinary Body Stop. Ctrl-C exits the process. Settings
are session-local and do not borrow Todo's checkpoint authority. The browser
consumes the semantic Face fragment; an owner-sealed Mask route and acknowledged
canonical Show remain separate integrations.

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

Every accepted browser action enters the same installed Body/Plan/Play. The
production scan invokes its exact transition child Back and commits the decoded
state; correlated child Signs retain their parent Play identity. Explicit Fore
close completes normally, while encounter teardown requests Body Stop. Face revisions advance even
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
