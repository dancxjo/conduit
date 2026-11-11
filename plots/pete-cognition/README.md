# Pete cognition

Each typed frame supplies a battery reading, human utterance, safety state,
and reference instant with explicit source signs. The Pete Situation Gear
selects the Situation and constructs the bounded Experience input for the
episode composer. The model and memory cell run in the same ordinary Play.

The current Plot invokes Situation selection and Experience integration through
`pete/situate-experience`. `body/episode-compose-experience` selects retained
episodes and builds the model request. Wiring the generic Experiencer and RAG
components as explicit Gears remains implementation work for the cognition MVP.

Situation selection also feeds fresh battery and safety readings through Pete's
homeostasis reducer. The result is labeled as deterministic derived evidence,
with the exact input signs and policy identity. Stale readings remain visible
as observations and leave current energy or motion unknown.

The composer retrieves from the four retained episodes using distinct shared
ASCII words of at least four characters, with a small common-word filter.
It searches retained observations and reports each match score in the model
request. Pete’s fact context includes recalled observations and historical
evidence. Typed episode records preserve earlier model-derived responses.
Higher scores and then newer episodes take precedence under the prompt bound.

The hosted proof registers Pete's Back on the selected Host, submits two frames,
and checks that the second model turn can use the retained first episode.
It also checks that a future observation fails before model admission.

The Plot opts into a typed provider-loss outcome on the generation Flow.
An unavailable model attempt advances the episode count and emits a record
without response text. The recall ring retains earlier successful episodes.
The next frame gets its own model attempt, with one call in flight and no
automatic retry of the lost attempt. A hosted fault-injection check verifies
that a later frame resumes recall in the same Play.

```sh
cargo xtask check pete-cognition
cargo xtask check pete-cognition-live
```

The live check uses installed Ollama models on loopback. The proof supplies
frame inputs through the Fore. The [resumable Plot](../pete-cognition-resumable/README.md)
also exercises a Body and Wake checkpoint across a replacement Host Boot,
Plan, and Play.

The Patchbay replay proof collects bounded debugger records from the installed
Host for the same two-episode Plot. It checks traffic on every authored Cord
and traces the second response through both input frames, first-episode
retention, later recall, and the inputs to the compose and retain Gears.
Patchbay follows a bounded set of observed input events at each join.

```sh
CONDUIT_PETE_REPLAY_SNAPSHOT="$PWD/target/pete-replay.json" \
  cargo test --locked -p xtask --test experience_episode_loop \
  pete_create_serial_samples_enter_one_cognition_play
cargo run --locked -p conduit-browser-patchbay-workbench -- \
  --replay-snapshot target/pete-replay.json
```

Open the printed loopback URL, then open Inspect to see the recorded Play in
the Signs view.
Live sensor and homeostasis sources remain part of
[#3707](https://github.com/dancxjo/conduit/issues/3707).

The Linux serial proof waits for the cognition Play to start, then uses the
std Create UART Base to read and submit the first correlated sample. After the
first episode is delivered, it captures and submits the second sample into the
same Play. The proof uses a pseudo-terminal by default. Set
`CONDUIT_PETE_SERIAL_OLLAMA=1` to use the installed Ollama model in that same
Play. To select a physical Create, pass its device path.

```sh
CONDUIT_PETE_CREATE_SERIAL=/dev/ttyUSB0 \
  cargo test --locked -p xtask --test experience_episode_loop \
  pete_create_serial_samples_enter_one_cognition_play
```

The probe starts and pauses a bounded sensor stream for each sample. It
supplies typed test utterances and carries unknown motion safety when a separate
safety observation is unavailable.

If a sensor read fails and the stream pauses successfully, the proof records
unavailable battery evidence with the Create source identity. The Play can
process the utterance and resume battery observation on a later frame.

Run the same Plot with your own utterances on a std Host:

```sh
cargo xtask prove pete std-cognition --retain \
  --replay-out target/pete-operator-replay.json \
  --episodes-out target/pete-operator-episodes.jsonl \
  --utterance 'The hallway switch label is COPPER KITE.' \
  --utterance 'What was the switch label?'
```

This command starts one Body and Wake for the supplied utterances and uses
the installed loopback Ollama model. Pass `--serial-path /dev/ttyUSB0` to
capture a Create sample for each utterance. Without a serial path, battery
state is unknown. Responses are printed as the Play delivers episodes.

Open the optional replay with the Workbench's `--replay-snapshot` argument.
The replay opens on the final episode's upstream trace and keeps the same
Body, Wake, Host, Plan, and Play identities as the operator run.

The optional episode archive writes one JSON line per delivered episode,
including the evidence, recalled episode numbers, and actual model outcome.
Each line also carries the canonical typed record bytes, including the recalled
evidence. The command synchronizes a row before submitting the next utterance.

To retain memory across invocations, reuse a checkpoint directory:

```sh
cargo xtask prove pete std-cognition --checkpoint-dir target/pete-memory --retain \
  --utterance 'The hallway switch label is COPPER KITE.'
cargo xtask prove pete std-cognition --checkpoint-dir target/pete-memory \
  --utterance 'What exact label did I report for the hallway switch?'
```

The second invocation starts a replacement Play with the same Body and Wake.
Episode numbers continue from the last committed checkpoint. One process at a
time can use a checkpoint directory. The command waits for the episode and its
checkpoint before submitting another utterance.
