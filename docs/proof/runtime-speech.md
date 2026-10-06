# Produce speech through the hosted runtime

The hosted `speech/synthesize` implementation can use an explicitly selected
local eSpeak NG engine. It passes text through the ordinary checked plot, plan,
kernel Host Call, bounded PCM conversion, and WAV artifact output. It opens no
audio device. The deterministic speech implementation remains a separate proof
implementation.

Select the executable, its actual engine library file, and the installed voice
data directory. For a typical x86-64 Linux installation:

Declare the exact portable Language explicitly and bind it to the selected
provider source before running speech. The private voice name does not determine
Language identity. This declaration is Host metadata, not a pronunciation proof:

```sh
cargo xtask make host declare-speech-language \
  --executable /usr/bin/espeak-ng \
  --data /usr/lib/x86_64-linux-gnu/espeak-ng-data \
  --engine /usr/lib/x86_64-linux-gnu/libespeak-ng.so.1.1.51 \
  --voice en-us --language language/english \
  --output target/english-speech-coverage.native \
  --request-output target/english-speech-request.native

cargo xtask make host prove-speech \
  --executable /usr/bin/espeak-ng \
  --data /usr/lib/x86_64-linux-gnu/espeak-ng-data \
  --engine /usr/lib/x86_64-linux-gnu/libespeak-ng.so.1.1.51 \
  --language-coverage target/english-speech-coverage.native \
  --language-request target/english-speech-request.native \
  --voice en-us --text 'Hello.' --output target/hello-speech
```

Use the real library filename installed on your machine, not a guessed version
or a symlink. The command requires a new output directory and retains a WAV and
its execution evidence. A successful artifact is produced audio; listening to
it is a separate action.

The provider hashes the selected executable, engine library, voice-data tree,
and fixed options. Planning binds that exact resource and explicit process
execution authority to the current host and boot. Text travels through stdin,
never a shell or command-line arguments. The OS loader and system libraries
remain part of the trusted host platform; this is not hostile-code confinement.

The single-shot portable limits remain 256 UTF-8 text bytes and 131,072 PCM bytes.
At the selected 22,050 Hz mono S16 profile, the audio ceiling is about three
seconds. Longer speech refuses with an output-capacity failure. It is never
silently truncated. Cancellation, timeout, unavailable or changed providers,
and malformed output remain distinct failures.
The process deadline covers engine execution and bounded retirement; the
bounded provider-file verification before and after it is a separate step.

For a longer utterance, add `--stream`. This authors the closing text Flow,
canonical language-aware segment commit, `speech/synthesize-stream`, PCM
conversion, and output with explicit aggregate work bounds. The entrance
accepts up to 1024 UTF-8 bytes and admits at most 32 segments, 1,323,000 source
PCM bytes, and 30 seconds of audio. For example, use the same provider arguments
with:

```sh
--stream --text 'This Body keeps the clock you started. Change the interval, then inspect the connections to see how your action reaches the running work. You can pause the Body without erasing its history. When you return, inspect the current host and the new plan before starting again. If a presentation host disappears, the Body must show what stopped and which admitted route can continue. Your preference chooses among available Masks; it never invents a missing host.' \
  --output target/streamed-speech
```

Text length alone cannot predict audio duration; exceeding an admitted audio
bound refuses rather than truncating. These larger aggregate allowances do not
increase instantaneous queue capacities. The engine's stdout is pulled in
bounded blocks, and the WAV sink writes an admitted temporary file incrementally.
Only completed output is published and acknowledged. Pressure stops draining;
cancellation retires the process and incomplete artifact. This command proves
the synthesis/output path, not a Body Face or a Mask Show.

The playback and conversion contracts also carry explicit block and audio-time
budgets. Browser playback accounts for mixed sample rates exactly within its
finite `rational128` profile. An exhausted arithmetic capacity is a distinct
storage refusal; it never rounds duration down or sends the rejected block to
the audio device.

The streamed Mask uses a separate closing-Flow projection from an already
validated outward Speech segment. Its 1024-byte semantic envelope and the
commit contract's revised envelope are explicit. The existing single-shot
Value projection remains distinct. A longer utterance does not mean unchecked
model token deltas may become speech.

To include runtime-produced speech in a current local-model documentary, add
`--speech-executable`, `--speech-data`, `--speech-engine`,
`--speech-language-coverage`, and optionally
`--speech-voice` to `cargo xtask make host prove-local-model` together with
`--orifina-presenter --journey-documentary`. This still requires an already-local
model and an explicit `--admitted-memory-mib` limit. The retained current
Presenter result passes through an ordinary streaming spoken Mask; its WAV is
bound to the acknowledged Show. Reusing one state's audio for several views of that
state does not claim another model inference or synthesis occurred.

A requested audio failure prevents sealing the documentary track. Without the
speech options, the existing deterministic proof route remains explicitly
identified and does not claim intelligible runtime speech.

The `cargo xtask prove one-body-spoken-chapter` producer likewise requires
`--speech-language-coverage` alongside its explicit executable, data, and engine
selection. Its retained English Face proof supplies an explicit English semantic
request; the selected voice name does not determine that request. Coverage is
rechecked against the exact current provider before the spoken Mask is planned.
