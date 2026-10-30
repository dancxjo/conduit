# Prove finite Linux microphone acquisition

Development work for #6606 adds `audio/capture-finite`, an explicitly requested
finite acquisition. It is distinct from a push-to-talk gesture and from the
whole-clip microphone entrance. This proof does not establish Body admission,
protected network delivery, or a shared Face projection.

The authored request contains capture duration and block size. The initialized
Host selects the executable, ALSA endpoint and exact-hardware or explicit ALSA
conversion implementation. The planner selects that offer with microphone
resource custody and explicit capture authority. Effects cross the ordinary
admitted Host Call boundary.

```sh
cargo xtask make host prove-microphone-flow \
  --arecord-executable /usr/bin/arecord --card-id CMTECK --device 0 \
  --alsa-plug --capture-milliseconds 3000 --block-frames 1024 \
  --timeout-seconds 10 --authorize-capture --json
```

Use the current card identity reported by the actual Host, not a numeric card
index. Without `--alsa-plug`, the selected hardware must support the exact
16 kHz mono signed 16-bit little-endian profile. Conversion is a different
initialized offer; this command does not call converted samples hardware-exact.
`--dry-run` validates finite request bounds without inspecting or opening a
microphone. Live use requires the explicit capture flag.

The command authors, checks, expands and plans a plot whose closing PCM Flow
leaves a sealed output Fore. The consumer validates sequence, capture clock,
frame offsets, block size and the exact short final block. Success requires the
exact total frame count, completed Plan terminal and unchanged admitted value
capacity. Receipts contain Plan/Play identity, provider identity, block/frame
counts, sample hash and consumer arrival timing; raw audio is not written.
First-delivery latency is not an acquisition timestamp or BodyTime claim.

The current profile admits 1–6000 milliseconds, 1–2048 frames per block and at
most 64 blocks. A request whose duration/block combination exceeds this work
bound refuses before acquisition. Every block carries a capture-scoped sample
clock reserved before Play, with offsets starting at zero. Clocks do not imply
wall time or continuity between captures.

Preparation reserves frame and diagnostic storage and observes bounded OS
pipes. During acquisition, each provider poll performs at most one bounded
read per pipe and does not sleep. ALSA's bounded setup diagnostics must confirm
the application-facing profile before samples escape. A retained block must be
consumed before another provider poll. Pressure, xrun/recovery, partial samples,
short capture, overflow, provider loss, unconfirmed format, cancellation and
timeout are explicit failures. No block is silently dropped or padded.

The original deadline includes executable revalidation, acquisition and pipe
drain after direct-provider exit. Cancellation and failures retire the isolated
provider process group, including inherited pipe writers. This is cooperative
Linux provider custody, not confinement of hostile native code or a hard
real-time guarantee. ALSA conversion internals remain trusted provider code.

Deterministic tests separately establish delivery of the first block before
provider completion, exact final blocks, pressure/failure behavior, inherited
writer cancellation/deadlines and allocation-free provider polling. A fixture
result does not substitute for running this command on a physical microphone.
