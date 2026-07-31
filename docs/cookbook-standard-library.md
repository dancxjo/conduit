# Standard Library Node Cookbook

This cookbook shows bounded source shapes for the `conduit-std` catalog. A
recipe is runnable only when its state below says so; a published contract is
not an installed provider. The checked-in inventory
[`examples/runnability.json`](../examples/runnability.json) is the
executable authority for example files.

---

## Catalog arrangement

Library names have one canonical path: `conduit.std/merge`, `time/delay`,
`state/cell`, `fs/read`, and `net/http/serve`. `std/...` is reserved for
fundamental values and mechanics such as `std/integer`, `std/text`,
`std/literal`, and `std/text/format`; it is not a bucket for every built-in
node.
The five multi-port structural components use the checked `conduit.std/...`
identities restored by issue #124. Their discarded `flow/...` spellings are
not aliases.

Finding a contract in the catalog proves only that its meaning is defined. A
host must separately advertise an implementation and finite limits, the plan
must carry any required grant, and placement must select an eligible host. For
example, `net/http/serve` is a valid contract-only node on an RP2040 or in a
browser even when neither host can provide or authorize it.

The standard type catalog includes mathematical `std/integer` and
`std/natural`, fixed-width `std/i8`–`std/i128` and `std/u8`–`std/u128`,
structural constructors such as `std/option`, `std/result`, `std/list`, and
`std/map`, operational types, and domain types such as
`net/http/request` and opaque `fs/resource` handles. Hosts advertise representation limits
separately; recognizing `std/integer` does not claim arbitrary-precision
storage.

Polymorphic standard nodes publish their relationships explicitly:
`flow/identity<T>` is `T -> T`, `conduit.std/tee<T>` is `T -> (T, T)`,
`conduit.std/merge<T>` is `(T, T) -> T`, `flow/first<T>` produces
`std/option<T>`, `flow/count<T>` produces `std/natural`, and both
`time/delay<T>` and `state/cell<T>` preserve `T`. Any concrete byte ports used
by reference fixtures are provider specializations, not a universal
byte-placeholder contract.

---

## 1. Text Formatting

State: **runnable** on the hosted profile (`format` and `display/text` have exact
installed bindings).

`std/text/format` is the ordinary typed `template + values -> text` filter.
It accepts automatic `{}`, indexed `{0}`, and named `{name}` placeholders;
`{{` and `}}` produce literal braces. `std/format-values` contains at most 32
ordered values with optional unique names and supports only bounded text,
boolean, and integer scalars. Missing or unused values, malformed
placeholders, unsupported kinds, and output overflow terminate with exact
`format/...` codes during execution.

```panel
panel 0

node template : std/literal {
    value = "{worker} processed {count} records; payload = {{ok}}.\n"
}
node values : std/format-values/literal {
    values = list(
        record(name="worker", value="worker-1"),
        record(name="count", value=42)
    )
}
node message : std/text/format
node sink : display/text

cord template.value -> message.template { capacity = 1 max_value_bytes = 4096 max_queued_bytes = 4096 low_watermark = 0 high_watermark = 1 pressure = block }
cord values.values -> message.values { capacity = 1 max_value_bytes = 16384 max_queued_bytes = 16384 low_watermark = 0 high_watermark = 1 pressure = block }
cord message.text -> sink.text { capacity = 1 max_value_bytes = 16384 max_queued_bytes = 16384 low_watermark = 0 high_watermark = 1 pressure = block }
```

The exact grammar, type descriptors, wire representation, limits, normalized
terminal codes, migration rule, and conformance fixture are current in
[specification 054](../spec/054-text-format.md).

### Lines and finite join

State: **runnable** with the hosted exact-plan provider.

`std/text/lines` removes LF or CRLF delimiters, preserves empty logical lines,
and emits a final unterminated line. `std/text/join` waits for a finite bounded
item stream, retains order, and inserts its configured separator only between
items. Their retained and output sizes are explicit configuration and plan
bounds; neither performs Unicode normalization or locale-sensitive work.

See the checked [`lines → join`](../examples/text-lines-join.panel) and
[`format → lines`](../examples/format-lines.panel) compositions and
[specification 060](../spec/060-text-lines-join.md).

## 2. Structural & Flow Control Nodes

State: **runnable** for exact `std/text` instances on the hosted profile.
The installed provider set is still explicit: another host may report these
contracts as unsupported, and unsupported modes fail during resolution.

### Deterministic merge

`conduit.std/merge` preserves each input's order and uses a retained round-robin
cursor to choose between simultaneously ready inputs. The two inputs have
stable identities; source order and executor wake order are not policies.

The current checked panel uses `std/text/lines` to turn finite text batches
into exact text streams, merges those streams, and uses `std/text/join` to
return to a finite batch for stdout:

```sh
cargo run -p conduct -- --check examples/flow-merge.panel
cargo run -p conduct -- --explain examples/flow-merge.panel
cargo run -p conduct -- examples/flow-merge.panel
```

See [`examples/flow-merge.panel`](../examples/flow-merge.panel) for the
complete Panel 3 source.

### Coupled and isolated tee

`conduit.std/tee` in `coupled` mode publishes both branches in one executor
transaction. If either
branch blocks, neither branch commits and the input lease rolls back.
`isolated` retains at most one exact value and advances each output
independently; the retained value and both branch reservations are charged to
the node's exact execution profile.

The checked Panel 3 example makes the finite-batch/stream boundary explicit
with `std/text/lines` before tee and one bounded `std/text/join` per branch:

```sh
cargo run -p conduct -- --check examples/flow-tee.panel
cargo run -p conduct -- --explain examples/flow-tee.panel
cargo run -p conduct -- examples/flow-tee.panel
```

See [`examples/flow-tee.panel`](../examples/flow-tee.panel) for the complete
source.

### Zip, gate, and select

- `conduit.std/zip` pairs `left` and `right` and atomically publishes correlated
  `left` and `right` outputs. `unpaired = "fail"` rejects an early terminal
  remainder; `unpaired = "drop"` is the explicit lossy alternative.
- `conduit.std/gate` processes control before data in a step. `initial` is `open` or
  `closed`; the hosted profile supports `retained = "block"`, so a closed gate
  propagates pressure instead of hiding a retained value or loss.
- `conduit.std/select` processes `selector` before the selected input. `initial` is
  `left` or `right`; `inactive = "block"` preserves inactive-input pressure and
  rejects implicit loss.

Checked standalone panels:
[`zip`](../examples/flow-zip.panel),
[`gate`](../examples/flow-gate.panel), and
[`select`](../examples/flow-select.panel). The checked
[`select → tee` composition](../examples/flow-compose.panel)
demonstrates why the nodes are separate ordinary graph boundaries.

---

## 3. Explicit Time Nodes

State: **runnable** on the hosted exact-plan executor. `time/delay`,
`time/timeout`, `time/debounce`, and `time/throttle` pin the
`conduit.clock/monotonic-ticks` descriptor, schema, hash, and one-tick
resolution. Durations are finite, clock discontinuities fail closed, and each
node owns at most one timer and one retained value.

- delay chooses terminal `drain` or `drop`;
- timeout is an inactivity boundary reset by each value;
- debounce names leading or trailing admission, explicit coalescing, and
  terminal `flush` or `drop`;
- throttle is either lossless leading admission with `block`, or trailing
  admission with explicit `coalesce`.

Run the four standalone panels and their checked composition:

```sh
cargo run -p conduct -- examples/time-delay.panel
cargo run -p conduct -- examples/time-timeout.panel
cargo run -p conduct -- examples/time-debounce.panel
cargo run -p conduct -- examples/time-throttle.panel
cargo run -p conduct -- examples/time-compose.panel
```

The composition splits two logical lines, debounces to the trailing value, then
crosses a lossless leading throttle. Exact execution advances only to a
plan-retained timer deadline; it never reads ambient wall-clock state.

---

## 4. State & Memory Nodes

`state/cell`, `state/deduplicate`, and `state/cache` are **runnable** through
the hosted exact-plan executor. `state/counter` remains contract-only.

- cell pins its state schema, optional initial value, emission policy, one-value
  byte ceiling, reset-to-initial behavior, and restart/checkpoint policy;
- deduplicate pins collision-safe equality, finite entry and byte windows,
  FIFO eviction, explicit duplicate drop, reset, and empty restart;
- cache uses an exact text request schema for `put`, `get`, `invalidate`, and
  `reset`, plus finite key/value/total byte bounds and FIFO eviction. It has no
  ambient TTL, persistence, or automatic checkpoint.

```sh
cargo run -p conduct -- examples/state-cell.panel
cargo run -p conduct -- examples/state-deduplicate.panel
cargo run -p conduct -- examples/state-cache.panel
cargo run -p conduct -- examples/state-compose.panel
```

The composition suppresses repeated put/get requests before the cache. Both
state boundaries remain ordinary nodes with independent exact allocations and
evidence. Restart always returns cell to its configured initial state and
empties deduplicate/cache state.

---

## 5. Resilience & Supervision Nodes

### Retry, Backoff Policy, and Circuit Breaker

State: **runnable** with exact hosted providers.

```sh
cargo run -p conduct -- examples/supervision-retry.panel
cargo run -p conduct -- examples/supervision-circuit-breaker.panel
cargo run -p conduct -- examples/supervision-compose.panel
```

Retry consumes typed terminal observations and emits at most the configured
number of attempts against the same exact implementation, resources, and
grants. Fixed or capped exponential backoff is one plan-visible retry policy;
it is not a second node. Jitter requires an injected entropy value.

The breaker retains a finite outcome window, opens at its declared threshold,
waits on the injected clock, and admits only the configured half-open probes.
Committed effects, exhausted attempts, stale descriptors, missing entropy,
and unsupported checkpoint or restart policies fail closed.

---

## 6. Hardware & Wireless Nodes

### Wi-Fi Station Join

State: **contract-only** on a device profile; no device capability, permission,
grant, or Wi-Fi provider is implied.
```panel
panel 0

node sta : net/wifi/join { ssid = "OfficeNet" }
node status_logger : observe/log

cord sta.state -> status_logger.message { capacity = 4 max_value_bytes = 1024 max_queued_bytes = 4096 low_watermark = 1 high_watermark = 4 pressure = block }
```

### GPIO Hardware Control

State: **contract-only** on a device profile; no GPIO provider or actuation
grant is implied.
```panel
panel 0

node button : device/gpio/pin { pin = 4 mode = "read" }
node led : device/gpio/pin { pin = 13 mode = "write" }

cord button.state -> led.command { capacity = 4 max_value_bytes = 256 max_queued_bytes = 1024 low_watermark = 1 high_watermark = 4 pressure = block }
```

---

## 7. Verification Commands

Check contract-only source or run the separately verified runnable example:

```sh
# Verify source grammar and lowering
conduct check examples/network-health.panel

# Inspect lowered identities and node interface proofs
conduct explain examples/network-health.panel

# Run deterministic execution plan
conduct run examples/hello.panel
```
