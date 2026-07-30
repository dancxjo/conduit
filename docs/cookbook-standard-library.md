# Standard Library Node Cookbook

This cookbook shows bounded source shapes for the `conduit-std` catalog. A
recipe is runnable only when its state below says so; a published contract is
not an installed provider. The checked-in inventory
[`examples/runnability-v1.json`](../examples/runnability-v1.json) is the
executable authority for example files.

---

## Catalog arrangement

Library names are canonical domain paths: `flow/merge`, `time/delay`,
`state/cell`, `fs/read`, and `net/http/serve`. `std/...` is reserved for
fundamental values and mechanics such as `std/integer`, `std/text`,
`std/literal`, and `std/text/format`; it is not a bucket for every built-in
node.
The former `conduit.std/...` names are not aliases.

Finding a contract in the catalog proves only that its meaning is defined. A
host must separately advertise an implementation and finite limits, the plan
must carry any required grant, and placement must select an eligible host. For
example, `net/http/serve` is a valid contract-only node on an RP2040 or in a
browser even when neither host can provide or authorize it.

The standard type catalog includes mathematical `std/integer` and
`std/natural`, fixed-width `std/i8`–`std/i128` and `std/u8`–`std/u128`,
structural constructors such as `std/option`, `std/result`, `std/list`, and
`std/map`, operational types, and domain types such as
`net/http/request` and `fs/path`. Hosts advertise representation limits
separately; recognizing `std/integer` does not claim arbitrary-precision
storage.

Polymorphic standard nodes publish their relationships explicitly:
`flow/identity<T>` is `T -> T`, `flow/tee<T>` is `T -> (T, T)`,
`flow/merge<T>` is `(T, T) -> T`, `flow/first<T>` produces
`std/option<T>`, `flow/count<T>` produces `std/natural`, and both
`time/delay<T>` and `state/cell<T>` preserve `T`. Any concrete byte ports used
by reference fixtures are provider specializations, not a universal
byte-placeholder contract.

---

## 1. Text Formatting

State: **runnable** on the hosted profile (`format` and `stdout` have exact
installed bindings).

`std/text/format` is the ordinary typed `template + values -> text` filter.
It accepts automatic `{}`, indexed `{0}`, and named `{name}` placeholders;
`{{` and `}}` produce literal braces. `std/format-values` contains at most 32
ordered values with optional unique names and supports only bounded text,
boolean, and integer scalars. Missing or unused values, malformed
placeholders, unsupported kinds, and output overflow terminate with exact
`format/...` codes during execution.

```panel
panel 1

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
node sink : io/stdout

cord template.out -> message.template { capacity = 1 max_value_bytes = 4096 max_queued_bytes = 4096 low_watermark = 0 high_watermark = 1 pressure = block }
cord values.out -> message.values { capacity = 1 max_value_bytes = 16384 max_queued_bytes = 16384 low_watermark = 0 high_watermark = 1 pressure = block }
cord message.out -> sink.in { capacity = 1 max_value_bytes = 16384 max_queued_bytes = 16384 low_watermark = 0 high_watermark = 1 pressure = block }
```

The exact grammar, type descriptors, wire representation, limits, normalized
terminal codes, migration rule, and conformance fixture are frozen in
[specification 054](../spec/054-text-format-v1.md).

## 2. Structural & Flow Control Nodes

### Pass-Through & Merge

State: **contract-only**; no merge provider is installed.
```panel
panel 1

node src1 : std/literal { value = "stream_a\n" }
node src2 : std/literal { value = "stream_b\n" }
node merger : flow/merge
node sink : io/stdout

cord src1.out -> merger.in { capacity = 4 max_value_bytes = 1024 max_queued_bytes = 4096 low_watermark = 1 high_watermark = 4 pressure = block }
cord src2.out -> merger.in { capacity = 4 max_value_bytes = 1024 max_queued_bytes = 4096 low_watermark = 1 high_watermark = 4 pressure = block }
cord merger.out -> sink.in { capacity = 4 max_value_bytes = 1024 max_queued_bytes = 4096 low_watermark = 1 high_watermark = 4 pressure = block }
```

### Fan-Out Tee Node

State: **contract-only**; tee, log, and blob-store providers are not installed.
```panel
panel 1

node src : std/literal { value = "telemetry_event\n" }
node splitter : flow/tee
node logger : observe/log
node store : storage/blob/store { bucket = "events" }

cord src.out -> splitter.in { capacity = 8 max_value_bytes = 4096 max_queued_bytes = 32768 low_watermark = 2 high_watermark = 8 pressure = block }
cord splitter.out -> logger.in { capacity = 8 max_value_bytes = 4096 max_queued_bytes = 32768 low_watermark = 2 high_watermark = 8 pressure = block }
cord splitter.out -> store.in { capacity = 8 max_value_bytes = 4096 max_queued_bytes = 32768 low_watermark = 2 high_watermark = 8 pressure = block }
```

---

## 3. State & Memory Nodes

### Counter & Cell State

State: **contract-only**; counter and cell providers are not installed.
```panel
panel 1

node tick_gen : state/counter
node state_cell : state/cell { initial = "STATE_IDLE" }
node display : io/stdout

cord tick_gen.out -> state_cell.in { capacity = 4 max_value_bytes = 256 max_queued_bytes = 1024 low_watermark = 1 high_watermark = 4 pressure = block }
cord state_cell.out -> display.in { capacity = 4 max_value_bytes = 256 max_queued_bytes = 1024 low_watermark = 1 high_watermark = 4 pressure = block }
```

### Deduplication

State: **contract-only**; stdin and deduplication providers are not installed.
```panel
panel 1

node raw_events : io/stdin
node dedup : state/deduplicate
node sink : io/stdout

cord raw_events.out -> dedup.in { capacity = 16 max_value_bytes = 4096 max_queued_bytes = 65536 low_watermark = 4 high_watermark = 16 pressure = drop_disposable }
cord dedup.out -> sink.in { capacity = 16 max_value_bytes = 4096 max_queued_bytes = 65536 low_watermark = 4 high_watermark = 16 pressure = block }
```

---

## 4. Resilience & Supervision Nodes

### Circuit Breaker & Exponential Backoff

State: **contract-only**; breaker, timing, and HTTP client providers are not
installed.
```panel
panel 1

node request_src : std/literal { value = "query" }
node breaker : supervision/circuit-breaker
node backoff_retry : supervision/backoff
node client : net/http/fetch { endpoint = "https://api.example.com/v1" }

cord request_src.out -> breaker.in { capacity = 8 max_value_bytes = 2048 max_queued_bytes = 16384 low_watermark = 2 high_watermark = 8 pressure = block }
cord breaker.out -> backoff_retry.in { capacity = 8 max_value_bytes = 2048 max_queued_bytes = 16384 low_watermark = 2 high_watermark = 8 pressure = block }
cord backoff_retry.out -> client.in { capacity = 8 max_value_bytes = 2048 max_queued_bytes = 16384 low_watermark = 2 high_watermark = 8 pressure = block }
```

---

## 5. Hardware & Wireless Nodes

### Wi-Fi Station Join

State: **contract-only** on a device profile; no device capability, permission,
grant, or Wi-Fi provider is implied.
```panel
panel 1

node sta : net/wifi/join { ssid = "OfficeNet" }
node status_logger : observe/log

cord sta.out -> status_logger.in { capacity = 4 max_value_bytes = 1024 max_queued_bytes = 4096 low_watermark = 1 high_watermark = 4 pressure = block }
```

### GPIO Hardware Control

State: **contract-only** on a device profile; no GPIO provider or actuation
grant is implied.
```panel
panel 1

node button : device/gpio/pin { pin = 4 mode = "read" }
node led : device/gpio/pin { pin = 13 mode = "write" }

cord button.out -> led.in { capacity = 4 max_value_bytes = 256 max_queued_bytes = 1024 low_watermark = 1 high_watermark = 4 pressure = block }
```

---

## 6. Verification Commands

Check contract-only source or run the separately verified runnable example:

```sh
# Verify source grammar and lowering
conduct check examples/network-health.panel

# Inspect lowered identities and node interface proofs
conduct explain examples/network-health.panel

# Run deterministic execution plan
conduct run examples/hello.panel
```
