# Pinned bounded scan patch

Vendored from crates.io esp-radio0.18.0, archive SHA256
`23fbff98b06a96b6ce3791ecec5c668524052a068e23aacd23afe17ddba844ce`.
Upstream repository https://github.com/esp-rs/esp-hal, revision
`347003de8a48320bb7724f53045be3afa9204411`, package path `esp-radio`.
`UPSTREAM-SHA256.json` records every original package file's bytes digest. The
original crate's MIT OR Apache-2.0 license applies; license texts are included
from that exact upstream revision. Registry source is never modified.

The only original package file changed is `src/wifi/mod.rs`: export the new
bounded_scan module and replace scan SSID-filter Vec scratch with33 fixed bytes.
Legacy scan_async, SDK versions, SDK allocator and configuration remain intact.
New bounded_scan sources own the controller and persistent subscriber, retain
cancellation custody, preserve raw SDK errors/authentication/status/count/id,
limit caller output to16 records, and require actual terminal event plus list
cleanup before controller return. No selected-operation authority is provided.

Installed esp-wifi-sys-esp32=0.2.0 bindings identify
CONFIG_IDF_INIT_VERSION="5.5.3" (`src/include.rs:522`). The corresponding official
[ESP-IDF5.5.3 scan event contract](https://docs.espressif.com/projects/esp-idf/en/v5.5.3/esp32/api-guides/wifi.html#wifi-event-scan-done)
documents ScanDone after an explicit scan stop, but not for blocking/connect
scans. Stop return is not treated as terminal evidence. Start returns no scan
identifier; exclusivity and explicit initial-quiescence trusted construction
provide sequential correlation, while the actual terminal scan_id is retained.

Lag or an ambiguous SDK start error poisons publication for that invocation.
The persistent subscriber still reconciles a later actual terminal event;
cleanup returns custody only after that event and successful SDK list clear.
No terminal evidence means custody remains held. Dropping a nonquiescent
OwnedScan deliberately forgets its controller guard; this retains radio
quarantine rather than running an unproved SDK deinit.
Callers must retain the owner through cleanup. This is a fail-closed fault path,
not a normal recovery mechanism or evidence that a timed-out scan stopped.

Raw SDK auth values are intentional: upstream AuthenticationMethod::from_raw
maps unknown values to Open. This new entrance does not make that conversion.
The SDK itself may already report an unsupported suite as Open; scan observation
cannot strengthen that underlying evidence into authenticated compatibility.

No Cargo patch selection, native compilation or physical result is implied by
this source directory. The owning composition must select this exact source and
qualify its target build, arena/frame budget and real cancellation before use.

Portable custody/copy tests run directly with:
`rustc --edition2024 --test src/wifi/bounded_scan/custody.rs -o /tmp/scan-tests`

The SDK shim harness compiles the actual OwnedScan implementation against a
controlled CPU SDK/event boundary. It covers cancellation, raw status/count/auth,
stop and list-clear refusal, lag reconciliation and late completion after start
error. It is not real header ABI, hardware, mode restoration or target fit proof:
`rustc --edition2024 --test tests/bounded_scan_host.rs -o /tmp/scan-host-tests`
