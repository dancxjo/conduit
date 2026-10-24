# Shared HTTP response lifetime and physical Root capacity

Reading, setup-page and submission-acknowledgement bytes reuse each connection's
prepared response buffer. Rendering completes synchronously before socket awaits;
private input is still released only after the full response flush. This removes
4,080 bytes of static task state in the keyed image. The exact released capacity
moves to Root's internal heap: 118,768 bytes instead of 114,688. SDK capacity stays
73,728 bytes and the linked CPU stack stays 36,988 bytes, above its 36 KiB floor.
The static RAM/stack section extents remain unchanged; this is redistribution,
not additional physical RAM or a stack high-water measurement.

The attended run completed three physical DHT publications and native Face replay.
All three identities/revisions matched across 59 HTTP responses. Setup-page GET
returned 200 and one generated nonexistent-network POST returned 202 without
credential echo. Each source capture reported `awaiting-native-retirement`.
After worker retirement, one credentials call and one join call ran; association
was rejected and AP services were restored in the same Boot, with no success
receipt. This proves ordered sampling/setup/fallback, not simultaneous transition
admission or successful station attachment. No actual network credentials were used.

Root peaked at 79,692 bytes and stayed there through sampling, with 39,076 bytes
of unused capacity. The ordinary Type-bounded slot change saved 3,136 actual heap
bytes versus the preceding capture (3,157 payload bytes before allocator rounding).
The readiness harness initially rejected the new profile because of an old fixed
capacity assertion; that failure and verified recovery are retained separately.

Native work entered through `cargo xtask prove environment owner-face-run-esp32`;
supplemental HTTP/fixture observation used host Wi-Fi with Ethernet retained.
`cargo xtask prove environment http` passes 32 tests, and strict HTTP lint passes.
Full logs, ELF/map and metadata are outside Cargo output at
`outputs/environment-native-face-probe/1792872769`. The protected operational image
was restored and verified with three fresh sequences and twelve acknowledged
physical display updates. Browser membership, a recorder Line, acquisition time,
native continuing display and combined admission remain unfinished.
