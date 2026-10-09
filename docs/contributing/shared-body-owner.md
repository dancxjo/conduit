# Local Linux Body owner (development)

This is a local development owner prerequisite for #4807, not accepted release
support for a Body shared with browser or QEMU guests. One installed Linux Host
can birth a Body from checked source, plan it, run supported local effects, and
retain its biography across fresh Boots. Existing `conduit body birth` remains
the browser birth entrance.

## Install and enter

Use a reviewed release containing this development change. Repository release
production enters through:

```sh
cargo xtask make host release --platform linux --output target/creche-host-releases
```

The existing development installation entrance verifies and installs the release
without activating its user service:

```sh
conduit host service install target/creche-host-releases/hosted-linux-x86_64.json \
  --state-dir /absolute/path/to/host-state --no-start
```

To make a speaker and eSpeak NG provider part of the Host's **initial**
inventory, first inspect `conduit body speech-options --json`, then add the
exact `--selected-speech --speaker-card CARD --speaker-device DEVICE
--speech-executable PATH --speech-data PATH --speech-engine PATH
--speech-language-coverage PATH` values to
the install command. Repeat `--speech-engine` for each reported engine library
dependency; `--speech-voice` is optional. Create the artifact-bound native coverage
declaration through [the runtime speech entrance](../proof/runtime-speech.md). The selection is retained across
reinstalls. Each fresh Boot rechecks the selected equipment and provider
before publishing a runtime marker or admitting a Body. If they are absent or
changed, startup fails explicitly. Reinstall with a new selection to recover,
or use `--without-selected-speech` to remove it. Omit all speech selection flags to
preserve the installed choice. Use an isolated state directory when trying
this development entrance; a running service is not reconfigured in place.

For synthesis into retained WAV artifacts, choose `--selected-artifact-speech`
with the same explicit executable, data, engine and Language coverage arguments.
This mode requires no speaker card or device and is mutually exclusive with
`--selected-speech`. Each fresh Boot verifies the exact provider before offering
voice and artifact Backs. Direct spoken Show admission and acknowledgement still
use the Owner's ordinary Face and Mask route. The WAV proves synthesis; this mode
records no device playback or human listening. The current `read remaining`
playback entrance still requires a selected speaker.

The selected voice Boot admits a finite pool of 64 create-new, per-Play WAV
artifacts under `spoken-artifacts/`. Each admitted Play uses its own exact
artifact destination; a physical speaker route separately checks current device
availability and playback authority.

An already local Ollama model can be offered by that same installed Host Boot:
add `--selected-model MODEL --model-endpoint http://127.0.0.1:11434
--model-memory-mib 2048` to the install command. Selection checks the local
inventory and a bounded warmup; it never downloads a model. The installed
record retains the exact model offer. Each fresh Boot reobserves and initializes
that provider before publishing its Host advertisement; a changed model,
runtime, or offer refuses startup until an explicit reinstall selects it again.
Use `--without-selected-model` to remove the retained choice, or omit both
selection flags to preserve it. This supplies a real owner Host model offer;
an owner-selected LLM spoken Mask and its Show require a separately admitted
route and are not implied by the installation alone.

For a fresh installation, `--no-start` leaves the Host available for foreground
ownership. Omitting it preserves the normal service activation behavior. It does
not stop an already running service; use a fresh installation for this sequence.
The two entrances are mutually exclusive owners of the same installation.
Read `product_executable` in the installation's `installation.json`, and invoke
that exact installed executable (shown below as `INSTALLED_CONDUIT`). A different
binary is refused before a new Boot or Body is created. The owner compares the
actual running image through `/proc/self/exe` with the installed executable.

For example, save this as `hello.conduit`:

```conduit
plot hello {
  show: presentation/text
  "Hello." >> show
}.
```

```sh
"$INSTALLED_CONDUIT" body own hello.conduit \
  --state-dir /absolute/path/to/host-state --name 'My Body'
```

The source is checked by the ordinary parser and planner. Reopening requires the
same checked resident source; it does not import a proof biography as birth.
The state directory must already be an installed Host and must not belong to a
different joined Body.

## Continue as a live installed service

After `body own` closes, the installed executable can run
`host service run --state-dir /absolute/path/to/host-state`. It resumes the retained Body on the
service's **fresh Boot**, persists that transition, and holds exclusive Host and
Body ownership for its lifetime.
`body status --state-dir /absolute/path/to/host-state --json` then asks the authenticated local service
for its current Body session rather than inferring a live Boot from
`runtime.json`. A stale marker alone cannot issue a live invitation.

While the service runs, `body invite` with `--route-bind`, `--route-url`,
`--route-tls-cert`, `--route-tls-key`, `--authorize-route`, and `--state-dir`
uses that service's single-use invitation authority. The matching `body join`
on another installed, running Host sends a signed request over the pinned TLS
route and retains the returned canonical receipt. The owner commits admission
to its current in-memory session and recoverable biography before sending the
receipt. A separate foreground `body own` or disk admission writer is refused
while the service owns the locks.

Membership admission alone does not establish continuing remote reachability.
The owner now selects separate current Face and interaction Lines for an
admitted native Mask, while an authorized browser window retains its own
presence and Mask route. Both validate the current Face and acknowledged Show
before a typed clock action returns. The retained membership snapshot by
itself is not a live lease, shared workload, or current remote Face; the
workload remains on the Linux owner in the local three-Host proof.

## Internal control and retained truth

The foreground entrance currently accepts bounded JSON lines on standard input.
This is an internal proof/control interface, **not a terminal Mask**. For a
bounded run, send these lines:

```json
{"operation":"plan"}
{"operation":"run","maximum_millis":1000}
{"operation":"inspect"}
{"operation":"close"}
```

`inspect`, `plan`, `run`, `lull`, and `close` are the supported operations.
`invite` with `ttl_seconds` from 1 through 600 issues the same single-use
portable Body invitation while this foreground owner holds the installation
lock; it retains the invitation before emitting the provisioning secret.
Ordinary control requests are at most 4096 bytes. The internal `admit-invited`
operation accepts at most 512 KiB because it carries one portable signed
`conduit.body/spawn-admission-request@1` document and an exact authorized
`expected_host_id`. It emits the canonical admission receipt only after the
single-use invitation and new membership biography have been retained together.
`admit-native-observation` accepts the JSON value emitted after
`CONDUIT_SPORE_JOIN ` by a provisioned ConduitOS guest in QEMU, plus the
independently authorized `expected_host_id`:

```json
{"operation":"admit-native-observation","expected_host_id":"<authorized HostId>","observation":{ "<exact ConduitOS serial observation>": "…" }}
```

Pass the complete observation object in place of the illustrative inner object;
the serial prefix is not JSON. The owner checks the observation contract and
Host/Boot correlation, then consumes the same single-use invitation through
the portable request boundary. This request also has the 512 KiB limit. The
operation never infers authorization from a serial line or silently admits a
Host merely because a request was printed.
Neither admission operation establishes or authenticates a carrier; its caller must
authorize the exact Host separately. A native guest must receive and validate
the receipt over its admitted return route before it may regard itself as joined.
Runs are synchronous and accept deadlines
from 1 through 60000 milliseconds; cancellation uses the production runtime's
cooperative stop control. Captured output is bounded to 32 KiB. There is no
concurrent interactive controller while a run is executing.

JSON output contains actual Host/Boot, Body biography, proposed realization, and
the last execution receipt. The receipt retains the exact Plan, Wake, Boot, and
actual Play/terminal Sign when execution started, including runtime failure or
cleanup failure. An untyped Host admission refusal is retained as such; it does
not invent typed resource evidence or a Play. The last receipt is historical
on reopen. Fresh Boot recovery preserves the Body and marks interrupted
realization honestly; it never restarts an old Play.

A committed Todo Face revalidates successful write/read terminal Signs against
both exact Plays, the restored state digest, selected content version and the
complete read residence (Host, Boot, base and profile). It also requires the
current canonical read Back and its authority requirements. A semantic content
contract alone grants no authority. Read failures retain the actual typed kernel
failure/detail and expose finite `todo-committed-*` refusal codes; diagnostic prose
is retained separately. Changing an unrelated Mask offer generation preserves a
valid immutable checkpoint witness.

The local JSON control envelope admits at most 2 MiB, including a browser
admission snapshot containing both Host advertisements. Remote session frames
and payloads retain their separate 512 KiB ceiling. A rejected JSON client frame
or failed response leaves the retained Owner service running; no action is
replayed after a transport failure.

Biography, installation binding, and bounded execution receipt are published
through a recoverable journal. Invalid or corrupt retained state is refused,
not replaced by a new Body. Exclusive locks also fence legacy biography and
membership mutations while the owner is running.

## Current boundaries

- Direct text presentation has real production-kernel coverage. Arbitrary
  advertised backs are not all supported by the Body dispatcher: `text/upper`
  can currently plan but is refused before Play. That refusal has a regression
  test; this slice does not claim complete local effect support.
- The owner can admit browser and ConduitOS participants, issue their separate
  Mask routes, and accept the checked clock controls over their actual Lines.
  The [local three-Host proof](native-three-host-proof.md) retains all three
  current Parts while the browser and QMP guest act. This does not establish
  workload migration, arbitrary remote actions, Mask preference, or a complete
  screen-free journey. The owner restores retained invitation authority after
  reopening; a retained Part is not automatically a current route or Show.
- The retained workset is fixed to the checked source at birth. This entrance
  does not yet provide source replacement or resident-workset editing.
- Biography compaction requires an admitted archive store. This slice has none
  and refuses when shared lifecycle archive capacity is reached; it does not
  silently discard evidence.
- Unit and local CLI smoke evidence are development evidence, not release,
  distributed, or human enactment acceptance for #4807.
