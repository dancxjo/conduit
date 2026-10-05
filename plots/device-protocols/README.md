# Portable device protocol composition

Implementation work for [#4833](https://github.com/dancxjo/conduit/issues/4833)
and reusable binary support for [#4831](https://github.com/dancxjo/conduit/issues/4831).
See the [device boundary and migration audit](../../docs/architecture/device-protocol-plots.md).

`binary.conduit` provides ordinary checked endian extraction, signed 8/16/24/32-bit
interpretation, bounded bitfield extraction, and explicit encoding overflow.
Packed words describe finite wire bytes; they are neither pointers nor resource
selectors. Encoding exposes separate `word` and `overflow` outputs. A caller
must handle overflow before using the word; the overflow path supplies no usable
encoded value. CRC-8 and reflected CRC-16 bit steps take their polynomial and
accumulator explicitly. They have no hidden device profile or retry policy.

`frames.conduit` supplies fixed four/eight-byte storage with explicit actual
length, concatenation and indexed access. Concatenation excludes source padding
and clears unused destination bytes; access distinguishes short input from a
malformed length. All branches are ordinary checked expressions. Preparation
for the current concatenation topology peaks below 2 MiB; repeated prepared
evaluation allocates nothing. This finite preparation cost is not a firmware
footprint claim. Prepared typed pairing retains nested record, variant and
nominal schemas instead of wrapping their canonical bytes as opaque leaves.
Its exact output profile differs from primitive-leaf pairing; ordinary checked
expressions can select nested tuple members without allocating during play.
These codecs and the portable zip Back are composition support, rather than an
installed native device lifecycle.

`i2c-types.conduit` defines exact finite requests and typed bus outcomes;
`i2c.conduit` invokes the class-neutral transaction Kind. The ConduitOS provider
binds the exact selected Plan/Play and opaque Base possession before effects.
Its I801 primitive supports SMBus send/receive byte and byte-data operations.
An independently validated ICH5-or-later profile additionally supports one
prefix byte followed by 2–32 I2C read bytes, with no SMBus count-byte assumption.
The native owner must establish block-read support, disabled auxiliary CRC and
buffer modes, and the actual SPD Write Disable configuration. Other transaction
geometries refuse before traffic. Native polling has one finite budget across
the complete block, with separately bounded stop work.
It preserves another controller semaphore owner, distinguishes controller error,
arbitration loss and timeout, and quarantines a controller that cannot stop.
An I801 device-error bit combines several hardware causes and is not reported
as a proved NACK. The physical port adapter requires admitted native ownership;
it does not discover, configure or advertise a controller by itself.

`bme280-compensation.conduit` contains ordinary checked temperature, pressure
and humidity arithmetic with explicit calibration and fine-temperature inputs.
Pressure exposes a separate invalid-calibration outcome for a zero denominator;
callers must handle it before using a numeric result. Humidity correction clamps
to the specified physical range. Compensation stages retain their exact declared
record schemas and use checked I128 arithmetic on every Host.

The canonical `main.conduit` decodes signed and unsigned calibration fields,
including packed signed twelve-bit humidity coefficients, and raw sample bytes.
`i2c-register.conduit` supplies source-owned read/write register request idioms.
Imported checked Types retain their refinement metadata. Construction may
forward an identical field contract or use a valid constant; it cannot silently
discard a law or justify arithmetic changes from shape alone.

`bme280-lifecycle.conduit` now checks and expands a staged probe/reset,
initialization, deadline and bounded polling policy, with calibration and sample
capture. Each event is handled once. The staged topology stays within the
existing expression-depth limit; it does not add a private protocol scheduler.
The lifecycle helpers expose closing event streams. A timed `waiting` action
carries the exact Source-owned monotonic deadline; `pending` means no new
effect while a transaction is in flight or a terminal observation was already
produced. A time provider must return its observed completion time, rather than
counting repeated ticks. Native admission tests now
execute initialization and action selection through the production kernel and
issue the source-authored identity read through the retained I2C owner. The
complete clock/feedback path also executes through the native kernel; packaged
Source enters the x86_64 product through Root admission and ordinary Plan/Play.
Deterministic transcripts check exact register order, response lengths, both
protocol addresses, refusal preservation, deadlines and finite poll exhaustion.
Prepared transition reuse allocates nothing. The source initializer owns the
initial state; the test harness supplies events and bus responses.

`bme280-feedback.conduit` connects Source initialization and transitions through
an explicitly declared, finitely retained state boundary. Every accepted event
uses the preceding committed state generation. The generic `flow/zip/feedback`
contract requires one initial state and one returned state per published pair;
event closure retains a pending event and waits for the last state return.
Ordinary `flow/zip/finite` keeps its existing unmatched-value discard behavior.
Native kernel tests exercise queued events, input closure during transition,
and normal completion with the exact retained state and pairing owners. This
proves Source feedback execution with supplied events. The autonomous entry
adds native bus/time event production and terminal closure.

`bme280-autonomous.conduit` now checks and expands the bus/time event topology
into 52 gears including decoding, compensation and terminal closure. Source adapters request exact wait deadlines, timestamp bus
results with the clock's observed time, and preserve clock failures separately.
The generic `flow/merge/finite` retains both input lifetimes and closes only
after both inputs drain. The graph plans against separately retained I2C and
clock offers with exact authority. Deterministic fixtures prepare, start and
cancel its native play without provider effects. The composite profile
admits at most 64 gears and 128 cords; its fixed scheduler is allocated during
preparation through the kernel’s allocated preparation entrance. A wrong-identity fixture executes a bus completion and clock observation through
the actual Source feedback loop, emits the exact refusal and issues no retry.
Each Cord has its own finite queue admission; an oversized clock queue is
rejected. A complete transcript fixture also executes probe, reset, configuration,
calibration reads, both deadline waits, sample capture and the exact fixed-point
temperature, pressure and humidity observation through those owners. Shared
Source decoding stages retain companion data without duplicated wire formulas.
Decoding and observation entrances use finite closing flows. Terminal closure
retires the native owners and canonical Body. Root admits the exact package,
controller, attachment, authority and clock before native execution.

The x86_64 clock provider reuses the calibrated invariant-TSC or ACPI HPET
counter with an explicit finite provider lifetime. Each poll observes the counter
once; unsupported deadlines, expiry and revocation remain distinct. Clock
availability is independent of networking. Counter conformance tests establish
conversion and refusal behavior, not hardware compatibility.

Its `bme280-observation` entry assembles one typed fixed-point temperature,
pressure and humidity observation from decoded calibration and samples.
Malformed input, disabled samples and invalid calibration yield explicit
unavailable results; their numeric scratch is never published. Its staged
topology uses checked arithmetic and capacity-stable prepared storage.

The reviewed inventory registers this reusable observation entry with workspace
discovery disabled. Its oracle proves checked arithmetic and prepared reuse;
it does not advertise a sensor provider or claim hardware execution.

The complete BME280 lifecycle runs through the production kernel with separately
admitted finite I2C and clock Backs. The supported `protocol-source`,
`protocol-input`, `protocol-image` and `protocol-run` xtask commands check Source,
prepare exact typed inputs, package over an existing capable kernel and observe
bounded native execution. See the [packaging and admission guide](../../docs/architecture/device-protocol-plots.md).
A retained freestanding x86_64 emulator run publishes exact Plan/Play identities,
emits the truthful controller refusal when no BME280 is attached, and retires.
Deterministic conformance separately proves successful observations, wrong
identity and clock loss. Physical BME280 compatibility remains unverified.

CRC check-vector tests compose the steps in a deterministic test harness; they
do not establish a complete authored CRC flow through the production kernel.

The repository entrance is:

```sh
cargo xtask check device-protocols
```

It checks/expands the source and exercises endian round trips, signed extremes,
bitfield boundaries, overflow and standard CRC check vectors with stable prepared
storage, exact possession/replay/revocation, finite controller termination and
complete BME280 native kernel lifecycle conformance. This suite is deterministic
proof; native emulator observation uses the separate supported `protocol-run`
entrance, and neither establishes physical compatibility.
