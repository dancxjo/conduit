# USB Source plots

These development plots separate received protocol data from possession of a
controller, device, endpoint or DMA resource. Descriptor and HID observations
confer no authority. Root admits exact physical owners separately.

`hid-reports.conduit` validates bounded received keyboard and mouse frames.
Keyboard reports normalize six usage slots before semantic state is advanced.
Error usages, duplicate usages, short frames and malformed extents remain
observable dispositions. Its zero-reserved entry is an explicitly stricter
profile, rather than a claim that the standard forbids all OEM fields.

`hid-keyboard-state.conduit` derives a fixed twenty-slot transition batch from
validated, normalized previous and current reports: modifier changes in bit
order, sorted releases, then sorted presses. Unchanged slots are explicit.

`hid-keyboard-lifecycle.conduit` retains previous-report state and drains each
batch through generic state, zip and merge Backs before pairing the next
command. Its command entry finishes explicitly; cancellation is a distinct
kernel outcome. Its received-frame entry connects the checked decoder and
normalization to that loop. Invalid reports are observed without replacing
previous state. That entry remains live after frame EOF and currently requires
Root cancellation to retire; graceful device teardown remains incomplete.

Repository validation enters through:

```sh
cargo xtask make conduitos usb-plots-check
```

Deterministic fixtures exercise 64 report generations and 1,274 ordered
transitions, held-output pressure, explicit finish, cancellation, and malformed
wire reports through the production kernel. Execution uses storage reserved
before Play and allocates nothing during Play. These are kernel/conformance
proofs. They establish neither an installed USB class offer nor interrupt
endpoint execution, physical compatibility, or five-architecture USB emulator
acceptance. Those remain work under #4831.
