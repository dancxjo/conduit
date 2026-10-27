//! Actual hardware diagnostic composition of the common environmental Face/Look.
//! No installed Body, admitted Plan, signed history, or membership is asserted.
extern crate alloc;
use conduit_bosch_env::{Address, Chip, Humidity, Reading, Sensor};
use conduit_rp2040_tm1637::PicoTm1637;
use conduit_tm1637_wire::{transmit, Packet};
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
#[derive(Clone, Copy)]
struct Snapshot {
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
    let s = critical_section::with(|cs| *SNAPSHOT.borrow(cs).borrow());
    let mut out = heapless::String::new();
    let (_, peak) = crate::heap::usage();
    let (chip, id) = match s.chip {
        Some(Chip::Bme280) => ("bme280", "96"),
        Some(Chip::Bmp280) => ("bmp280", "88"),
        None => ("uninitialized", "null"),
    };
    write!(&mut out,"{{\"role\":\"{}\",\"chip\":\"{}\",\"chip_id\":{},\"address\":118,\"samples\":{},\"attempts\":{},\"frames\":{},\"acks\":{},\"heap_peak\":{},\"status\":\"{}\",\"display\":\"{}\"",crate::protocol::ROLE,chip,id,s.samples,s.attempts,s.frames,s.acks,peak,s.error,s.display).map_err(|_|"sensor-status-capacity")?;
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
    let mut pins = PicoTm1637::new(clock, data);
    let mut last_sample = None;
    let mut revision = 0u64;
    let mut delay = Delay;
    loop {
        if last_sample.is_none_or(|at: Instant| at.elapsed() >= Duration::from_secs(30)) {
            update(|s| s.attempts = s.attempts.saturating_add(1));
            let initialized = if sensor.chip().is_none() {
                matches!(
                    with_timeout(Duration::from_millis(100), sensor.initialize(&mut delay)).await,
                    Ok(Ok(_))
                )
            } else {
                true
            };
            let reading = if initialized {
                with_timeout(Duration::from_millis(100), sensor.measure(&mut delay))
                    .await
                    .ok()
                    .and_then(Result::ok)
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
        let s = critical_section::with(|cs| *SNAPSHOT.borrow(cs).borrow());
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
        let result = transmit(
            &mut pins,
            &[
                Packet { bytes: &[0x40] },
                Packet { bytes: &data },
                Packet { bytes: &[0x8a] },
            ],
            || false,
        );
        update(|s| {
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
