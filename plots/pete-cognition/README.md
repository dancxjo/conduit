# Pete cognition

Each typed frame supplies a battery reading, human utterance, safety state,
and reference instant with explicit source signs. The Pete Situation Gear
selects the Situation and prepares an `EpisodeExperienceInput` for the generic
`experience/integrate-episode` Gear. That Gear admits the whole revision through
the existing `CurrentExperience` reducer, checking freshness, sources, capacities,
and relationships before publishing the episode's selected Experience.

`body/episode-retrieval-window` captures the exact current experience and
resident memory packets with Body/Wake ownership, the original reference instant,
and the Plot's explicit intent, clock class and context policy. The existing
`retrieval/temporal-select-flow` resolves source times; missing, incomparable,
invalid and future times remain explicit omissions. Episode counters never
become timestamps.

`body/episode-retrieval-context` verifies that outcome against the same snapshot,
reuses the episode loop's shared-term retrieval law, uses the existing bounded
context selector, and atomically publishes the model
request and native pending episode. Only selected complete historical evidence
items enter the request. Earlier trigger text, unselected archive items and
prior model words stay in native custody; original observations and actual
outcomes remain available for audit. Selection ordinals are not relevance scores.
The model, retainer and memory cell run in the same ordinary Play.

The Plot authors an event-ordering intent, the explicit shared-term ranking
profile, a one-episode history limit and a 2 KiB selected-history ceiling. The
profile selects a matching complete evidence item, prefers factual reports to
followup questions, and separately ranks typed model-attempt outcomes. Missing
matches are explicit exclusions; they are not fabricated relevance scores.
The declared charge is one unit per rendered UTF-8 byte, not measured model
tokens. Instructions, current evidence, history and omission diagnostics must
all fit the unchanged 4 KiB model request; overflow refuses atomically. The
160 KiB capture and 64 KiB temporal-window Cords are prepared transport bounds,
not increases to the four-entry, 8 KiB autobiographical memory.

The model data uses the versioned `episode-retrieval-request@2` format. Repeated
classification and instant field names are declared once beside exact column
values. An exact repeated utterance can refer to `observation.content`; its
JSON-string encoding is resolved during retention. The native record keeps the
same transmitted current projection, original observation and complete actual
outcome. Model history carries the actual disposition and identifies its Host
source; complete attempt/run provenance stays in the paired native audit.
Retained time columns remain attached to their values across checkpoint recovery.
Retrieval uses the supplied frame reference as its as-of instant.

Situation selection also feeds fresh battery and safety readings through Pete's
homeostasis reducer. The result is labeled as deterministic derived evidence,
with the exact input signs and policy identity. Stale readings remain visible
as observations and leave current energy or motion unknown.

The hosted proof registers Pete's Back on the selected Host, submits two frames,
and checks that the second model turn can use the retained first episode.
It also checks that a future observation fails before model admission.

The Plot opts into a typed provider-loss outcome on the generation Flow.
An unavailable model attempt advances the episode count and emits a record
without response text. A selected observation and its exact failed-attempt
metadata remain in the recall ring. The next frame gets its own model attempt,
with one call in flight and no automatic retry of the lost attempt. Hosted
fault-injection checks verify later recall of both earlier successful episodes
and the observation made during provider loss, including after checkpoint
recovery. The model context labels the failed attempt as host-reported outcome
data. The Plot supplies the trusted instruction; historical evidence is data and
cannot replace it.

```sh
cargo xtask check pete-cognition
cargo xtask check pete-cognition-live
```

The live check uses installed Ollama models on loopback. The typed-frame tests
accept `CONDUIT_PETE_LIVE_MODEL` and `CONDUIT_PETE_LIVE_MEMORY_MIB` to select an
already installed model and its explicit memory admission; their defaults remain
`gemma3:latest` and 6144 MiB. Model answer quality is configuration-specific, and
these options change neither the assertions nor the cognition bounds. The proof supplies
frame inputs through the Fore. The [resumable Plot](../pete-cognition-resumable/README.md)
also exercises a Body and Wake checkpoint across a replacement Host Boot,
Plan, and Play.

The Patchbay replay proof collects bounded debugger records from the installed
Host for the same two-episode Plot. It checks traffic on every authored Cord
and traces the second response through both input frames, first-episode
retention, later recall, and the inputs to the recall, contextualize, and retain Gears.
Patchbay follows a bounded set of observed input events at each join.
The debugger retains 128 records. Longer runs expose eviction and missing-parent
gaps; they do not claim an exact trace outside that retained horizon.

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

For independent attention updates and explicit reflection positions, use the
[three-Form composition](../pete-cognition-cadenced/README.md) with
`--reflection-after`. Its source, model bounds, checkpoint retention, and
Patchbay trace are verified by the same cognition checks.
