# Local Linux Body owner (development)

This is the first local owner prerequisite for #4807, not accepted release
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

Biography, installation binding, and bounded execution receipt are published
through a recoverable journal. Invalid or corrupt retained state is refused,
not replaced by a new Body. Exclusive locks also fence legacy biography and
membership mutations while the owner is running.

## Current boundaries

- Direct text presentation has real production-kernel coverage. Arbitrary
  advertised backs are not all supported by the Body dispatcher: `text/upper`
  can currently plan but is refused before Play. That refusal has a regression
  test; this slice does not claim complete local effect support.
- The owner can admit a browser participant during a bounded lulled window and
  consume either a preissued portable native request or the exact signed
  observation emitted by a real QEMU guest. The ConduitOS product does not yet
  complete that request-and-receipt exchange over a live duplex Line. Shared
  presentation, remote execution, and a terminal Mask are not implemented by
  this entrance. The owner restores the same retained single-use invitation
  authority after reopening.
- The retained workset is fixed to the checked source at birth. This entrance
  does not yet provide source replacement or resident-workset editing.
- Biography compaction requires an admitted archive store. This slice has none
  and refuses when shared lifecycle archive capacity is reached; it does not
  silently discard evidence.
- Unit and local CLI smoke evidence are development evidence, not release,
  distributed, or human enactment acceptance for #4807.
