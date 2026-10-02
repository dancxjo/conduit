# Produce speech through the hosted runtime

The hosted `speech/synthesize` implementation can use an explicitly selected
local eSpeak NG engine. It passes text through the ordinary checked plot, plan,
kernel Host Call, bounded PCM conversion, and WAV artifact output. It opens no
audio device. The deterministic speech implementation remains a separate proof
implementation.

Select the executable, its actual engine library file, and the installed voice
data directory. For a typical x86-64 Linux installation:

```sh
cargo xtask make host prove-speech \
  --executable /usr/bin/espeak-ng \
  --data /usr/lib/x86_64-linux-gnu/espeak-ng-data \
  --engine /usr/lib/x86_64-linux-gnu/libespeak-ng.so.1.1.51 \
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

The existing portable limits remain 256 UTF-8 text bytes and 131,072 PCM bytes.
At the selected 22,050 Hz mono S16 profile, the audio ceiling is about three
seconds. Longer speech refuses with an output-capacity failure. It is never
silently truncated. Cancellation, timeout, unavailable or changed providers,
and malformed output remain distinct failures.
The process deadline covers engine execution and bounded retirement; the
bounded provider-file verification before and after it is a separate step.

To include runtime-produced speech in a current local-model documentary, add
`--speech-executable`, `--speech-data`, `--speech-engine`, and optionally
`--speech-voice` to `cargo xtask make host prove-local-model` together with
`--orifina-presenter --journey-documentary`. This still requires an already-local
model and an explicit `--admitted-memory-mib` limit. The retained current
Presenter result passes through an ordinary spoken Mask; its WAV is bound to
the acknowledged Show. Reusing one state's audio for several views of that
state does not claim another model inference or synthesis occurred.

A requested audio failure prevents sealing the documentary track. Without the
speech options, the existing deterministic proof route remains explicitly
identified and does not claim intelligible runtime speech.
