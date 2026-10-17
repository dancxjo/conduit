# Keep one Todo list across Masks

The recorded Groceries list starts with Buy milk, Prepare lunch and Water
plants. Completing Buy milk in the terminal leaves two things to do. The
browser checklist, compact terminal and spoken summary choose different
presentations of that same retained Body and committed item state.

The complete capture is pinned to
`0f6b89e931e34d8b49b8f1aefedfaa1272b8ef65`. Its eight chapters and separately
linked receipts are published at `/conduit/journeys/current/todo/` after the
protected release train accepts the publication. Until those gates pass, the
checked-in packet is local execution evidence, not a deployed acceptance claim.

## Before you begin

Use a clean checkout of the pinned source, Rust from its `rust-toolchain.toml`,
a native Linux linker, Node.js, and the repository's pinned Playwright 1.62.0
with Chromium. [Browser proof setup](../../proof/browser/README.md) describes
the supported `cargo xtask prove browser-host` entrance and prerequisite checks.
The full Linux release set also compiles AArch64; prepare its cross-toolchain
with `cargo xtask setup linux-release` and check `cargo xtask doctor linux-release`.
Compilation of the ARM package does not demonstrate execution on an ARM machine.

Explicitly select an available eSpeak NG executable, data directory, engine
library and voice. The recording used eSpeak NG executable 1.52.0,
`libespeak-ng.so.1.1.51`, system eSpeak NG data and `en-us`. Its admitted provider
closure SHA-256 was
`de52918836888529207368fae51d4e2485ef9ded3df0da5321497acd00771514`.
A different admitted installation has its own provider identity and output.
The producer does not substitute a voice or speaker when selected equipment
is unavailable.

This example deliberately selects WAV artifact output. The capture machine
had no available ALSA speaker. All recordings are actual outputs of their
acknowledged Plays; none claims committed speaker delivery or human listening.
The browser and Owner are distinct semantic Hosts on the same physical machine.

## Prepare the release and Handbook

Check out the pinned source without editing it, then run:

```sh
cargo xtask make host release --platform linux --output "$PWD/target/todo-release"
cargo xtask make body static \
  --application targets/browser/handbook/handbook.application.template.json \
  --handbook --output "$PWD/target/todo-handbook"
```

Both output directories must be new. The release, Handbook runtime, Handbook
UI and producer checkout must name the same exact clean source commit.

Set these paths to your explicitly selected local provider:

```sh
TODO_ESPEAK=/absolute/path/to/espeak-ng
TODO_DATA=/absolute/path/to/espeak-ng-data
TODO_ENGINE=/absolute/path/to/libespeak-ng.so
```

Declare the English mapping against those exact provider bytes:

```sh
cargo xtask make host declare-speech-language \
  --executable "$TODO_ESPEAK" --data "$TODO_DATA" --engine "$TODO_ENGINE" \
  --voice en-us --language language/english \
  --output "$PWD/target/todo-english.native" \
  --request-output "$PWD/target/todo-english-request.native"
```

## Install a new private Host

Choose a new private short directory and create an empty checkpoint directory:

```sh
TODO_RUN=/tmp/conduit-todo-run
mkdir -m 700 "$TODO_RUN"
mkdir -m 700 "$TODO_RUN/checkpoint"
"$PWD/target/todo-release/conduit-linux-x86_64" host service install \
  "$PWD/target/todo-release/hosted-linux-x86_64.json" \
  --state-dir "$TODO_RUN/state" --no-start \
  --selected-todo-checkpoint-root "$TODO_RUN/checkpoint" \
  --selected-artifact-speech \
  --speech-executable "$TODO_ESPEAK" --speech-data "$TODO_DATA" \
  --speech-engine "$TODO_ENGINE" --speech-voice en-us \
  --speech-language-coverage "$PWD/target/todo-english.native"
```

Installation prints the release bundle SHA-256. Set `TODO_OWNER` to
`$TODO_RUN/state/releases/BUNDLE_HEX/conduit-linux-x86_64`, using the printed
digest without its `sha256:` prefix. Use this installed executable rather than
the build-tree executable. On Linux, the complete canonical
`$TODO_RUN/state/control.sock` pathname must be at most 107 bytes; the producer
refuses a longer pathname before Birth. The capture directory may use a longer
workspace path.

## Capture the actual journey

Use the pinned Playwright installation prepared for this checkout:

```sh
cargo xtask prove todo-journey \
  --state-dir "$TODO_RUN/state" --conduit-bin "$TODO_OWNER" \
  --output "$PWD/target/todo-capture" \
  --fresh-body-source plots/todo/checkpoint-once.conduit \
  --handbook-package "$PWD/target/todo-handbook" \
  --pinned-playwright "$PWD/proof/browser/node_modules/@playwright/test/index.mjs" \
  --first-item-text 'Buy milk' --cross-mask-actions --direct-speech
```

The producer births Groceries, acknowledges its terminal Show, adds the three
items through ordinary browser controls, and observes the initial list through
direct speech. It joins in the terminal, completes Buy milk through the exact
offered action, refuses the old browser action, and refreshes the checklist.
The final spoken opening says “2 things left · 1 completed”; explicitly
requested detail reads Prepare lunch and Water plants once each, in order.
This reader command discloses the current Face and does not mutate Todo state.

The producer then stops its owned service, verifies the same checkpoint under
a fresh Owner Boot and joins from a new admitted Browser Host without Birth.
For the refusal specimen it damages only the checkpoint created by this new
private run, retains `todo-committed-corrupt` without verified state, restores
the exact original bytes, and verifies the repaired same-Body state again.
Do not point this fresh proof at an existing user's checkpoint directory.

Success prints the `publication/` packet location only after the full event,
media and producer-terminal manifest validates. A failed command retains its
observations; inspect that failure before any further action. The packet's
manifest declares every output, byte length and SHA-256. Exact Body, Face,
Show, interaction, queue, Plan, Play and Sign witnesses stay in separately
linked evidence rather than the primary checklist or spoken words.

## Timing and bounds

The recorded actual journey took 15.387 seconds, excluding builds, installation
and equipment preparation. This is one local observation, not a realtime
guarantee. Its packet has 48 outputs totalling 4,203,685 bytes. Publication
admits at most 128 outputs, 16 MiB per file and 128 MiB per packet. The selected
artifact profile is stereo signed 16-bit PCM at 48 kHz, with independently
correlated Plays for the opening and each finite remaining-item batch.

Use this Todo journey as the application specimen when explaining rich Masks
or future WebXR presentations. Those presentations can choose different
rhetoric for the same Face; they must preserve the canonical item identities,
typed actions and committed-state witness. WebXR, a graphical guest and model
narration are not prerequisites for the browser/terminal/direct-speech journey.
