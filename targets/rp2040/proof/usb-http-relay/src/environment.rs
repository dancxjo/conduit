//! Actual hardware diagnostic composition of the common environmental Face/Look.
//! No installed Body, admitted Plan, signed history, or membership is asserted.
extern crate alloc;
use conduit_bosch_env::{Address, Chip, Humidity, Reading, Sensor};
use conduit_rp2040_tm1637::PicoTm1637;
use conduit_tm1637_wire::{transmit, OpenDrain, Packet};
use core::{cell::RefCell, fmt::Write};
use critical_section::Mutex;
use embassy_rp::{
    bind_interrupts,
    i2c::{self, I2c},
    peripherals::{I2C1, PIN_3, PIN_4, PIN_6, PIN_7},
    Peri,
};
use embassy_time::{with_timeout, Delay, Duration, Instant, Timer};
bind_interrupts!(struct SensorIrqs { I2C1_IRQ => i2c::InterruptHandler<I2C1>; });
#[derive(Clone)]
struct Snapshot {
    address: u8,
    primary_init: heapless::String<64>,
    secondary_init: heapless::String<64>,
    measurement: heapless::String<64>,
    display_error: heapless::String<64>,
    levels: (bool, bool),
    before: (bool, bool),
    samples: u64,
    attempts: u64,
    frames: u64,
    acks: u64,
    reading: Option<Reading>,
    chip: Option<Chip>,
    error: &'static str,
    display: &'static str,
}
static SNAPSHOT: Mutex<RefCell<Snapshot>> = Mutex::new(RefCell::new(Snapshot {
    address: 0x76,
    primary_init: heapless::String::new(),
    secondary_init: heapless::String::new(),
    measurement: heapless::String::new(),
    display_error: heapless::String::new(),
    levels: (false, false),
    before: (false, false),
    samples: 0,
    attempts: 0,
    frames: 0,
    acks: 0,
    reading: None,
    chip: None,
    error: "warming",
    display: "warming",
}));
fn update(f: impl FnOnce(&mut Snapshot)) {
    critical_section::with(|cs| f(&mut SNAPSHOT.borrow(cs).borrow_mut()));
}
pub fn status() -> Result<heapless::String<384>, &'static str> {
    let s = critical_section::with(|cs| SNAPSHOT.borrow(cs).borrow().clone());
    let mut out = heapless::String::new();
    let (_, peak) = crate::heap::usage();
    let (chip, id) = match s.chip {
        Some(Chip::Bme280) => ("bme280", "96"),
        Some(Chip::Bmp280) => ("bmp280", "88"),
        None => ("uninitialized", "null"),
    };
    write!(&mut out,"{{\"role\":\"{}\",\"chip\":\"{}\",\"chip_id\":{},\"address\":{},\"samples\":{},\"attempts\":{},\"frames\":{},\"acks\":{},\"heap_peak\":{},\"status\":\"{}\",\"display\":\"{}\"",crate::protocol::ROLE,chip,id,s.address,s.samples,s.attempts,s.frames,s.acks,peak,s.error,s.display).map_err(|_|"sensor-status-capacity")?;
    if let Some(r) = s.reading {
        let mut rh = heapless::String::<16>::new();
        match r.humidity {
            Humidity::PercentQ10(v) => write!(&mut rh, "{v}"),
            Humidity::Unavailable(_) => write!(&mut rh, "null"),
        }
        .map_err(|_| "sensor-status-capacity")?;
        write!(&mut out,",\"temperature_centi_celsius\":{},\"pressure_centi_pascal\":{},\"humidity_percent_q10\":{}",r.temperature_centi_celsius,r.pressure_centi_pascal,rh).map_err(|_|"sensor-status-capacity")?;
    }
    out.push('}').map_err(|_| "sensor-status-capacity")?;
    Ok(out)
}
fn describe<T, E: core::fmt::Debug>(
    value: &Result<Result<T, E>, embassy_time::TimeoutError>,
) -> heapless::String<64> {
    let mut out = heapless::String::new();
    match value {
        Ok(Ok(_)) => out.push_str("ok").unwrap(),
        Ok(Err(e)) => {
            let _ = write!(&mut out, "{e:?}");
        }
        Err(_) => out.push_str("deadline-100ms").unwrap(),
    };
    out
}
fn describe_wire<E: core::fmt::Debug>(
    value: &Result<(), conduit_tm1637_wire::Failure<E>>,
) -> heapless::String<64> {
    let mut out = heapless::String::new();
    match value {
        Ok(()) => out.push_str("ok-9-acks").unwrap(),
        Err(e) => {
            let _ = write!(&mut out, "{e:?}");
        }
    };
    out
}
pub fn diagnostic() -> Result<heapless::String<512>, &'static str> {
    let s = critical_section::with(|cs| SNAPSHOT.borrow(cs).borrow().clone());
    let mut out = heapless::String::new();
    write!(&mut out,"{{\"role\":\"{}\",\"primary_0x76\":\"{}\",\"secondary_0x77\":\"{}\",\"measure\":\"{}\",\"display_error\":\"{}\",\"idle_clock_high\":{},\"idle_data_high\":{},\"after_clock_high\":{},\"after_data_high\":{},\"settled_us\":1000,\"half_period_us\":50,\"weak_pullups\":true}}",crate::protocol::ROLE,s.primary_init,s.secondary_init,s.measurement,s.display_error,s.before.0,s.before.1,s.levels.0,s.levels.1).map_err(|_|"diagnostic-capacity")?;
    Ok(out)
}
// Only slows the electrical half-period; delegates every pin transition and
// actual ACK sample to the existing provider, never synthesizing an ACK.
struct SlowDiagnostic(PicoTm1637<'static>);
impl OpenDrain for SlowDiagnostic {
    type Error = core::convert::Infallible;
    fn clock(&mut self, released: bool) -> Result<(), Self::Error> {
        self.0.clock(released)
    }
    fn data(&mut self, released: bool) -> Result<(), Self::Error> {
        self.0.data(released)
    }
    fn read_data(&mut self) -> Result<bool, Self::Error> {
        self.0.read_data()
    }
    fn half_period(&mut self) -> Result<(), Self::Error> {
        embassy_time::block_for(Duration::from_micros(50));
        Ok(())
    }
}
#[embassy_executor::task]
pub async fn run(
    bus: Peri<'static, I2C1>,
    scl: Peri<'static, PIN_7>,
    sda: Peri<'static, PIN_6>,
    clock: Peri<'static, PIN_3>,
    data: Peri<'static, PIN_4>,
) {
    let mut config = i2c::Config::default();
    config.frequency = 100_000;
    let mut sensor = Sensor::new(
        I2c::new_async(bus, scl, sda, SensorIrqs, config),
        Address::Primary,
    );
    let mut provider = PicoTm1637::new(clock, data);
    // Attended 3.3 V diagnostic profile only; ordinary provider defaults stay unchanged.
    provider.set_internal_pullups(true);
    let mut pins = SlowDiagnostic(provider);
    let mut last_sample = None;
    let mut revision = 0u64;
    let mut delay = Delay;
    loop {
        if last_sample.is_none_or(|at: Instant| at.elapsed() >= Duration::from_secs(30)) {
            update(|s| s.attempts = s.attempts.saturating_add(1));
            let mut initialized = sensor.chip().is_some();
            if !initialized {
                // Move the same actual bus owner; never duplicate an I2C peripheral.
                sensor = Sensor::new(sensor.into_inner(), Address::Primary);
                let first =
                    with_timeout(Duration::from_millis(100), sensor.initialize(&mut delay)).await;
                update(|s| {
                    s.primary_init = describe(&first);
                    s.secondary_init.clear();
                    s.address = 0x76;
                });
                initialized = matches!(first, Ok(Ok(_)));
                if !initialized {
                    sensor = Sensor::new(sensor.into_inner(), Address::Secondary);
                    let second =
                        with_timeout(Duration::from_millis(100), sensor.initialize(&mut delay))
                            .await;
                    update(|s| {
                        s.secondary_init = describe(&second);
                        s.address = 0x77;
                    });
                    initialized = matches!(second, Ok(Ok(_)));
                }
            }
            let reading = if initialized {
                let measured =
                    with_timeout(Duration::from_millis(100), sensor.measure(&mut delay)).await;
                update(|s| s.measurement = describe(&measured));
                measured.ok().and_then(Result::ok)
            } else {
                None
            };
            last_sample = Some(Instant::now());
            if let Some(reading) = reading {
                if let Some(next) = revision.checked_add(1) {
                    revision = next;
                    update(|s| {
                        s.samples = revision;
                        s.reading = Some(reading);
                        s.chip = sensor.chip();
                        s.error = "available";
                    });
                }
            } else {
                update(|s| {
                    s.reading = None;
                    s.chip = sensor.chip();
                    s.error = "sensor-failed";
                });
            }
        }
        let s = critical_section::with(|cs| SNAPSHOT.borrow(cs).borrow().clone());
        let registers = match s.reading {
            Some(r) => crate::environment_show::render(
                r,
                revision,
                last_sample.map_or(30_000, |at| at.elapsed().as_millis()),
            )
            .unwrap_or([0x40, 0x40, 0x40, 0x40, 0, 0]),
            None => [0x40, 0x40, 0x40, 0x40, 0, 0],
        };
        let data = [
            0xc0,
            registers[0],
            registers[1],
            registers[2],
            registers[3],
            registers[4],
            registers[5],
        ];
        let before = pins.0.levels();
        let result = transmit(
            &mut pins,
            &[
                Packet { bytes: &[0x40] },
                Packet { bytes: &data },
                Packet { bytes: &[0x8a] },
            ],
            || false,
        );
        // Observe actual released input levels after a bounded RC settling period.
        embassy_time::block_for(Duration::from_millis(1));
        let levels = pins.0.levels();
        update(|s| {
            s.levels = levels;
            s.before = before;
            s.display_error = describe_wire(&result);
            s.frames = s.frames.saturating_add(1);
            if result.is_ok() {
                s.acks = s.acks.saturating_add(1);
            }
            s.display = if result.is_ok() {
                "acknowledged"
            } else {
                "wire-refused"
            }
        });
        Timer::after(Duration::from_secs(2)).await;
    }
}
