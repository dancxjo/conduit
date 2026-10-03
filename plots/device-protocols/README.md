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
footprint claim.

`i2c-types.conduit` defines exact finite requests and typed bus outcomes;
`i2c.conduit` invokes the class-neutral transaction Kind. The ConduitOS provider
binds the exact selected Plan/Play and opaque Base possession before effects.
Its I801 primitive supports SMBus send/receive byte and byte-data operations,
refuses other transaction geometries before traffic, and bounds native polling.
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
Deterministic transcripts check exact register order, response lengths, both
protocol addresses, refusal preservation, deadlines and finite poll exhaustion.
Prepared transition reuse allocates nothing. The source initializer owns the
initial state; the test harness supplies events and bus responses.

Its `bme280-observation` entry assembles one typed fixed-point temperature,
pressure and humidity observation from decoded calibration and samples.
Malformed input, disabled samples and invalid calibration yield explicit
unavailable results; their numeric scratch is never published. Its staged
topology uses checked arithmetic and capacity-stable prepared storage.

The reviewed inventory registers this reusable observation entry with workspace
discovery disabled. Its oracle proves checked arithmetic and prepared reuse;
it does not advertise a sensor provider or claim hardware execution.

These remain arithmetic, finite frame and bus foundations. They do not yet
supply an integrated production device lifecycle or an installed hardware Back.
CRC check-vector tests compose the steps in a deterministic test harness; they
do not yet establish a complete authored CRC flow through the production kernel.

The repository entrance is:

```sh
cargo xtask check device-protocols
```

It checks/expands the source and exercises endian round trips, signed extremes,
bitfield boundaries, overflow and standard CRC check vectors with stable prepared
storage, exact possession/replay/revocation and finite controller termination. These checks do not claim ConduitOS bus execution or hardware support.
