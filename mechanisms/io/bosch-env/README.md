# Bosch environmental sensor register driver

`conduit-bosch-env` is a small `no_std`, allocation-free driver for the actual
BME280 (`0x60` chip ID) and production BMP280 (`0x58`) on I2C address `0x76` or
`0x77`. It creates no Conduit Source, offer, authority, membership or hardware
proof. A target must retain the initialized bus/sensor and admit its own effects.

```rust,ignore
let mut sensor = Sensor::new(actual_i2c, Address::Primary);
// The target must wrap BOTH calls in its real bus/whole-operation timeout.
let chip = sensor.initialize(&mut actual_delay).await?;
let reading = sensor.measure(&mut actual_delay).await?;
```

The caller supplies `embedded-hal-async` I2C and `DelayNs`. There are at most eight
status reads per readiness wait, separated by 2ms, and a 10ms delay for x1 forced
conversion. This does not make an indefinitely pending bus future bounded: the
target must supply a real transport/whole-operation deadline. Failed or cancelled
initialization/measurement clears readiness and retains the bus. Initialize
again before measuring. No continuous background task, heap or clock is owned.

Outputs name their units explicitly:

- `temperature_centi_celsius`: hundredths of one degree Celsius.
- `pressure_centi_pascal`: hundredths of one Pascal.
- `Humidity::PercentQ10`: 1024ths of one percent relative humidity, 0..=102400.
- BMP280 yields `Humidity::Unavailable(UnsupportedChip)` and never reads/writes
  the BME humidity registers. It cannot supply an RH reading.

Factory calibration includes correct signed 12-bit H4/H5 and signed H6. Conversion
uses one coherent 6-byte BMP or 8-byte BME burst. The documented reset/skipped raw
sentinels are conservatively refused. Invalid factory blocks, zero pressure
divisor, arithmetic overflow and out-of-range temperature/pressure return an
error instead of fabricated min/max readings. Humidity's physical 0..100% clamp
follows Bosch. Wide integer intermediates keep malformed calibration from causing
Rust overflow; the host test covers 20,000 arbitrary register sets.

The normalized pressure unit follows the **current** Bosch integer64 SensorAPI
(hundredths Pa), rather than the older datasheet example's Q24.8 Pa. CPU tests
check the datasheet example within 0.05Pa and compare all three normalized outputs
exactly against 2,048 trajectories through pinned official C routines. The C
oracle is host-test-only and is never linked into firmware; `cargo test` needs a
host C compiler named `cc` for that differential test. Protocol tests also prove
IDs/register order, repeated-start burst lengths, finite busy refusal,
cancellation/reinitialization, retained bus custody and zero runtime allocation.

Primary sources:

- [Bosch BME280 datasheet](https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bme280-ds002.pdf)
- [Bosch BMP280 datasheet](https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bmp280-ds001.pdf)
- [Bosch SensorAPI source, pinned commit](https://github.com/boschsensortec/BME280_SensorAPI/blob/c90d419492e26dd95586598a794e65eb2760753a/bme280.c)

Compensation formulas and the differential oracle are adapted/copied from Bosch
Sensortec's BSD-3-Clause SensorAPI. The original copyright, conditions and
disclaimer are retained in `LICENSE-BOSCH` and the oracle. This CPU proof is not a
claim that a particular module is wired, powered, detected or operating.
