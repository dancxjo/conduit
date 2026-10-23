# Installed Host preparation phases (#6283)

The internal service diagnostic is enabled only by
`CONDUIT_HOST_STARTUP_TRACE=1`. Default startup remains silent apart from its
existing output. The trace emits at most 40 fixed-schema scalar records on
stderr, with the child PID, sequence and monotonic elapsed/duration microseconds.
It records neither paths nor installation contents, tokens or authority. Write
errors are best effort; timing includes instrumentation and scheduling.

Each phase wraps the existing operation in the same order and returns its
original result. Refusal or unwinding cannot emit successful preparation. The
`prepared` record precedes the existing stdout announcement; neither establishes
the later control-listener bind or admission. The existing live rendezvous and
browser admission assertions remain authoritative.

The browser fixture records stderr independently of stdout readiness. Its
16 KiB capture, 512-byte line and 40-record bounds reject truncated or malformed
success, wrong PID/sequence, missing mandatory phases and invalid ordering. It
retains a valid incomplete prefix on forced termination. Cleanup waits for pipe
close, escalates a live child after the existing grace period and retains a
bounded refusal for inherited pipes. The 20-second absolute fixture budget,
one worker, zero retries and product assertions are unchanged.

## Measured internal diagnostic

[Retained checks and raw logs](evidence/native-startup-phases-20261023/e91376129/checks.json)
bind clean native source `e913761296b11efc776ef1060acb9e17560fc8a4` and actual
CLI SHA256 `a19cfe5e51f0355e416e9ffec5bd739677439c3e306e53ad3b48170fcab3b779`.
The real single-worker Chromium rendezvous case passed without retries, with 32
valid native records. Host preparation measured 4957.331ms, including 3015.861ms
in catalogs, 1833.149ms in Birth encounter preparation and 87.447ms in Host
construction. The whole encounter measured 9086.238ms. This was one local run
under concurrent host builds, not calibrated CPU attribution or a reproduction
of the original over-budget CI run. The fixture's reviewed-image file is a
small text fixture; its digest is not a digest of the actual CLI executable.

The native CLI was freshly compiled. Browser assets were the retained
`work/6267-review/target/workspace-product`, not rebuilt or relabeled as current
source. This is internal real-browser diagnostic evidence, not fresh full-product
acceptance. It establishes that the live tracing/capture path works and gives
local phase observations; it does not identify the earlier CI stall's cause.

Forty owning Node controls pass. One actual CLI control passes on clean source:
absent, `0` and `true` settings are silent; `1` records the same failing startup,
with unchanged exit/original stderr and installation bytes, no runtime marker,
and no successful preparation record. Strict Clippy for the product binary and
that test target passes with warnings denied; workspace formatting passes. The
initial integration-test module-path compile failure is retained beside the
corrected success. Independent Rust and Node source reviews found no blocker.

#6283 remains open. Required delivery evidence is the merged diagnostic,
current native phase observations and the relevant supported fresh-product
browser Integration. Earlier 10-second CI failures and the subsequent bounded
20-second fixture correction remain their own source-bound evidence. No new
performance optimization, universal startup bound, readiness authority or issue
closure follows from this local diagnostic.
