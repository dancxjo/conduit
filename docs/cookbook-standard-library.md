# Standard Library Node Cookbook

This cookbook shows bounded source shapes for the `conduit-std` catalog. A
recipe is runnable only when its state below says so; a published contract is
not an installed provider. The checked-in inventory
[`examples/runnability-v1.json`](../examples/runnability-v1.json) is the
executable authority for example files.

---

## 1. Text Formatting

State: **runnable** on the hosted profile (`format` and `stdout` have exact
installed bindings).

`conduit.std/format` uses Rust-style positional `{}` placeholders. Parameters
are consumed from a finite text array in order; `{{` and `}}` produce literal
braces. Missing or unused parameters and unmatched braces are rejected while
the panel is resolved.

```panel
panel 1

node message : conduit.std/format {
    template = "{} processed {} records; payload = {{ok}}.\n"
    parameters = list("worker-1", "42")
}
node sink : conduit.std/stdout

cord message.out -> sink.in { capacity = 1 max_value_bytes = 1024 max_queued_bytes = 1024 low_watermark = 0 high_watermark = 1 pressure = block }
```

## 2. Structural & Flow Control Nodes

### Pass-Through & Merge

State: **contract-only**; no merge provider is installed.
```panel
panel 1

node src1 : conduit.std/literal { value = "stream_a\n" }
node src2 : conduit.std/literal { value = "stream_b\n" }
node merger : conduit.std/merge
node sink : conduit.std/stdout

cord src1.out -> merger.in { capacity = 4 max_value_bytes = 1024 max_queued_bytes = 4096 low_watermark = 1 high_watermark = 4 pressure = block }
cord src2.out -> merger.in { capacity = 4 max_value_bytes = 1024 max_queued_bytes = 4096 low_watermark = 1 high_watermark = 4 pressure = block }
cord merger.out -> sink.in { capacity = 4 max_value_bytes = 1024 max_queued_bytes = 4096 low_watermark = 1 high_watermark = 4 pressure = block }
```

### Fan-Out Tee Node

State: **contract-only**; tee, log, and blob-store providers are not installed.
```panel
panel 1

node src : conduit.std/literal { value = "telemetry_event\n" }
node splitter : conduit.std/tee
node logger : conduit.std/log
node store : conduit.std/blob-store { bucket = "events" }

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

node tick_gen : conduit.std/counter
node state_cell : conduit.std/cell { initial = "STATE_IDLE" }
node display : conduit.std/stdout

cord tick_gen.out -> state_cell.in { capacity = 4 max_value_bytes = 256 max_queued_bytes = 1024 low_watermark = 1 high_watermark = 4 pressure = block }
cord state_cell.out -> display.in { capacity = 4 max_value_bytes = 256 max_queued_bytes = 1024 low_watermark = 1 high_watermark = 4 pressure = block }
```

### Deduplication

State: **contract-only**; stdin and deduplication providers are not installed.
```panel
panel 1

node raw_events : conduit.std/stdin
node dedup : conduit.std/deduplicate
node sink : conduit.std/stdout

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

node request_src : conduit.std/literal { value = "query" }
node breaker : conduit.std/circuit-breaker
node backoff_retry : conduit.std/backoff
node client : conduit/http-client { endpoint = "https://api.example.com/v1" }

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

node sta : conduit.std/wifi-station { ssid = "OfficeNet" }
node status_logger : conduit.std/log

cord sta.out -> status_logger.in { capacity = 4 max_value_bytes = 1024 max_queued_bytes = 4096 low_watermark = 1 high_watermark = 4 pressure = block }
```

### GPIO Hardware Control

State: **contract-only** on a device profile; no GPIO provider or actuation
grant is implied.
```panel
panel 1

node button : conduit.std/gpio-pin { pin = 4 mode = "read" }
node led : conduit.std/gpio-pin { pin = 13 mode = "write" }

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
