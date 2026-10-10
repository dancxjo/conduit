# Thermostat

`thermostat/main` is an ordinary resident Plot beside Todo. Its finite scan owns
settings and observations; a Body Owner projects its contribution into the same
canonical Face used by other Plots. The existing browser Mask plans and plays that
Face, acknowledges its Show, and returns typed interactions to the current scan
Fore. Thermostat has no HTTP server, JavaScript application, or separate Body owner.

## Encounter in an installed Body

Use the exact executable installed for a fresh local Host, as described in
[shared Body ownership](../../docs/contributing/shared-body-owner.md). Birth the
Body with this checked resident source, then run its normal owner service:

```sh
conduit body own plots/thermostat/main.conduit \
  --state-dir /absolute/path/to/host-state --name 'Living room' < /dev/null
conduit host service run --state-dir /absolute/path/to/host-state
```

Open the ordinary [Handbook owner participation](../../targets/browser/handbook/README.md)
entrance (`?participate=owner`), admit that browser through the owner's normal
`body browser-window` command, and select its browser Mask at rest before starting
the Play. From another terminal, start one bounded Play:

```sh
conduit body start --state-dir /absolute/path/to/host-state --maximum-millis 900000
conduit body face --state-dir /absolute/path/to/host-state --json
```

Refresh its Face after starting. The shared `<conduit-owner-face>`
Web Component uses a reusable CSS Look and native radio controls for Face-declared
exclusive choices. It interprets generic roles, typed values, names, availability,
and selection; it knows no thermostat action prefixes or decicelsius properties.
Every accepted control updates the same Body Plan and Play. A stale Show refuses.

The Owner's mechanical spoken reader consumes the same Face and returns the same
typed interactions. Exact temperature values are normalized
`value/exact-decimal-quantity@1` data with the Celsius unit, not disguised Counts.
Generic graphical and spoken wording uses the value's checked kind and unit.
Live sensor, device speech playback during this scan, and physical heating or
cooling are not established by this slice. Selected speech equipment cannot be
silently attached to this scoped scan Host.

```sh
conduit body lull --state-dir /absolute/path/to/host-state
```

Lull stops the Play and revokes its contribution and controls. A subsequent Play
starts with the authored initial state. Settings are session-local; Thermostat
borrows no Todo checkpoint authority.

## Finite command-line Play

The regular product entrance also plays the checked Plot in a sealed BodyPlan:

```sh
printf '%s\n' \
  '{"command":"set-mode","mode":"heat"}' \
  '{"command":"set-target","target":225}' \
  '{"command":"set-fan","fan":"on"}' \
  | conduit run plots/thermostat/main.conduit \
      --report /tmp/thermostat-run.json \
      --artifacts /tmp/thermostat-artifacts
```

The command-line input codec uses signed tenths of a degree Celsius (`225` means
22.5°C), independently of the Face's exact Celsius quantity codec. Modes are
`off`, `heat`, `cool`, `auto`; fans `auto`, `on`; presets `comfort`, `eco`, `sleep`.
Each JSON line is bounded to 1024 bytes. EOF closes the Fore and completes the
Play; invalid input stops it. The report and new artifact directory retain the
checked source Plan, sealed BodyPlan, actual Play, terminal Sign, child Signs,
and observed outputs. Placement and Body overrides remain unavailable on this
local command-line scan route.

## Domain and proof

The state Form is exactly 13 bytes and each command is three bytes. A Play admits
at most 256 controls. Targets range from 10–30°C in 0.5°C steps; observations from
−50–80°C in 0.1°C steps. Off/Heat/Cool/Auto, Auto/On fan requests, and
Comfort/Eco/Sleep presets are semantic choices. Comfort/Eco/Sleep request
21/18/19°C in Off, Heat, Auto, or 24/26/25°C in Cool. A manually adjusted target
becomes Custom. Named presets adjust when mode changes. Invalid encodings,
exhausted revisions, and out-of-range values refuse; unchanged settings are
idempotent. Face revisions advance even for accepted no-ops to retire old Shows.

With no observation, the Face says **Sensor unavailable**. Requested heating and
cooling never claim confirmed equipment operation. No physical equipment or
sensor is connected, and the interface cannot fabricate an observation.

```sh
cargo xtask prove journey thermostat --verify
cargo xtask check plots check
```

The focused proof covers portable domain contracts, typed Face values, native
Plan/Play, and the installed Owner's same-Play mechanical spoken interaction.
The browser proof driver `proof/browser/thermostat-owner-browser.mjs` consumes an
already running installed Owner and a packaged Handbook. It uses the existing
production SDK and browser WASM Mask, one pinned Chromium worker, and no retries.
Its receipt distinguishes local development evidence from accepted releases,
physical behavior, and attended human evidence.

The [browser component audit](../../docs/architecture/browser-component-audit.md)
records which parts of #5376 this shared component already satisfies and the
remaining selected-Back, Look, fallback and accessibility acceptance.
