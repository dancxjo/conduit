# Environmental Source Host integration

Combined source: `0fe137c62e1a8e24869a641cd579ff7b9df70ba8`, including dev
`5bd8e9d94e`. This is CPU/process qualification, not physical delivery.

Passed on the combined integration tree:

- `cargo +stable test --locked --manifest-path bodies/environment/host/Cargo.toml --features carrier-source,retained-history-provider --lib -j 2`: 19 tests.
- Same package/features with `--test source_network -- --test-threads=1`: one actual confined-worker, two-device loopback test, including transfer of the actual Host and retained equipment through a real worker thread.

The tests exercise the actual global ready Base inventory, unique acquired
sessions, exact fragment/generation reservation witnesses, duplicate-capacity
refusal, retained provider custody, affine detach/resume, same collector
Host/Boot clock, and original canonical observation/history bytes. Source
failure does not publish a successful observation or mint replacement custody.

The reusable pinned-Root Face verifier and its existing client separately pass
three exact-content/scope API tests and all five original Face-client tests
(commit `9efc8b4c15`). This extracts the existing protocol; no new transport,
renewed lease, recording acknowledgment or membership grant is introduced.

Not established: installation in a running durable Host, selected same-Body
recorder commitment, a protected Source Line, MCU membership, actual device
source capture through this owner, live four-device Face publication, overnight
continuity, or physical flash/pulse timing. The confined endpoints are explicit
CPU fixtures. Existing physical carrier firmware and local displays were not
changed by this qualification.

## Follow-on qualification

At `787a5eeb74`, the existing signed-Face retrieval loop delegates to the shared
`read_snapshot_with` API. Four signed protocol tests and all five original
Face-client regression tests passed. This retains the original local deadline,
full document digest and execution basis; it adds no transport or authority.

At `6e121ca3d3`, the typed serial/network attachment prerequisite is integrated.
Its originating Make qualification passed all 86 library tests; the combined
Source/history library suite passed all 19 tests at this integration commit.
The declarations carry finite scalar configuration and retain existing device
identity and duplicate-Base rules. They do not acquire resources by themselves.

The subsequent merge of dev `644262694a` brings in the independent checked
atan2 quantity-difference repair. The preceding test counts are tied to the
commits above, not claimed as rerun after this merge. Installed Source execution,
physical deployment and same-Body durable recording remain unproved here.
