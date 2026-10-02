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
and activates its user service:

```sh
conduit host service install target/creche-host-releases/hosted-linux-x86_64.json \
  --state-dir /absolute/path/to/host-state
```

Stop that installed Host service before entering the foreground owner. The two
entrances are mutually exclusive owners of the same installation. On Linux the
service is `conduit-host.service`; use the platform service manager to stop it.
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
Each request is at most 4096 bytes. Runs are synchronous and accept deadlines
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
- Browser/QEMU guest admission, Lines, shared presentation, and a terminal Mask
  are not implemented by this entrance. Existing invitation mutations cannot
  run concurrently with the owner.
- The retained workset is fixed to the checked source at birth. This entrance
  does not yet provide source replacement or resident-workset editing.
- Biography compaction requires an admitted archive store. This slice has none
  and refuses when shared lifecycle archive capacity is reached; it does not
  silently discard evidence.
- Unit and local CLI smoke evidence are development evidence, not release,
  distributed, or human enactment acceptance for #4807.
