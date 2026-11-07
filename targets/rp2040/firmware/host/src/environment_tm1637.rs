//! Autonomous diagnostic quantities/Face/Look; no installed Body or Plan claim.
use crate::config;
use conduit_bosch_env::{Address, Humidity, Reading, Sensor, SensorState};
use core::{cell::RefCell, fmt::Write};
use critical_section::Mutex;
use embassy_rp::{
    i2c::{self, I2c},
    Peri,
};
use embassy_time::{with_timeout, Delay, Duration, Instant, Timer};
#[derive(Clone, Copy)]
struct Snapshot {
    samples: u64,
    sampled_at_ms: u64,
    attempts: u64,
    frames: u64,
    acks: u64,
    reading: Option<Reading>,
    chip: Option<conduit_bosch_env::Chip>,
    sensor_error: bool,
    display_error: bool,
}
static STATE: Mutex<RefCell<Snapshot>> = Mutex::new(RefCell::new(Snapshot {
    samples: 0,
    sampled_at_ms: 0,
    attempts: 0,
    frames: 0,
    acks: 0,
    reading: None,
    chip: None,
    sensor_error: false,
    display_error: false,
}));
fn update(f: impl FnOnce(&mut Snapshot)) {
    critical_section::with(|cs| f(&mut STATE.borrow(cs).borrow_mut()))
}
pub fn status() -> Result<heapless::String<1024>, ()> {
    let s = critical_section::with(|cs| *STATE.borrow(cs).borrow());
    let mut out = heapless::String::new();
    let (_, peak) = crate::heap::usage();
    let (resident_count, resident_bytes, resident_refusal) = crate::resident::summary();
    let chip_id = match s.chip {
        Some(conduit_bosch_env::Chip::Bme280) => 96,
        Some(conduit_bosch_env::Chip::Bmp280) => 88,
        None => 0,
    };
    write!(&mut out,"{{\"role\":\"{}\",\"build\":\"{}\",\"configuration\":\"{}\",\"sensor_address\":{},\"display_address\":null,\"samples\":{},\"attempts\":{},\"frames\":{},\"acks\":{},\"heap_peak\":{},\"sensor_error\":{},\"display_error\":{},\"body_member\":{}",config::ROLE,crate::BUILD_ID,config::CONFIGURATION_ID,config::SENSOR_ADDRESS,s.samples,s.attempts,s.frames,s.acks,peak,s.sensor_error,s.display_error,crate::membership::is_member()).map_err(|_|())?;
    write!(&mut out, ",\"display\":\"tm1637\",\"chip_id\":{chip_id}").map_err(|_| ())?;
    write!(&mut out, ",\"initialized_resident_backs\":{resident_count},\"resident_back_bytes\":{resident_bytes},\"resident_refusal_entry\":").map_err(|_| ())?;
    match resident_refusal {
        Some(entry) => write!(&mut out, "{entry}"),
        None => write!(&mut out, "null"),
    }
    .map_err(|_| ())?;
    write!(&mut out, ",\"sampled_at_boot_ms\":{}", s.sampled_at_ms).map_err(|_| ())?;
    if let Some(r) = s.reading {
        write!(&mut out,",\"temperature_centi_celsius\":{},\"pressure_centi_pascal\":{},\"humidity_percent_q10\":",r.temperature_centi_celsius,r.pressure_centi_pascal).map_err(|_|())?;
        match r.humidity {
            Humidity::PercentQ10(h) => write!(&mut out, "{h}"),
            Humidity::Unavailable(_) => write!(&mut out, "null"),
        }
        .map_err(|_| ())?;
    }
    crate::membership::append_status(&mut out)?;
    out.push('}').map_err(|_| ())?;
    Ok(out)
}
#[embassy_executor::task]
pub async fn run(
    bus: Peri<'static, config::Bus>,
    scl: Peri<'static, config::Scl>,
    sda: Peri<'static, config::Sda>,
    clock: Peri<'static, config::DisplayClock>,
    data: Peri<'static, config::DisplayData>,
) {
    let mut bus_config = i2c::Config::default();
    bus_config.frequency = config::FREQUENCY;
    let address = if config::SENSOR_ADDRESS == 0x76 {
        Address::Primary
    } else {
        Address::Secondary
    };
    crate::i2c_owner::initialize(conduit_rp2040_i2c::Rp2040I2c::new(
        I2c::new_async(bus, scl, sda, config::SensorIrqs, bus_config),
        config::MAXIMUM_I2C_POLLS,
    ));
    let mut sensor_state = SensorState::new(address);
    let mut protocol_revision = crate::i2c_owner::protocol_revision();
    let mut pins = crate::tm1637_pins::Pins::new(clock, data);
    let mut last_sample = None;
    let mut prepared = None;
    let mut delay = Delay;
    loop {
        let Some(mut bus) = crate::i2c_owner::try_lease() else {
            Timer::after_millis(40).await;
            continue;
        };
        let current_revision = crate::i2c_owner::protocol_revision();
        if current_revision != protocol_revision {
            sensor_state.invalidate();
            last_sample = None;
            protocol_revision = current_revision;
        }
        let mut sensor = Sensor::from_parts(&mut *bus, sensor_state);
        if last_sample.is_none_or(|at: Instant| at.elapsed() >= Duration::from_secs(30)) {
            update(|s| s.attempts = s.attempts.saturating_add(1));
            let ready = if sensor.chip().is_none() {
                matches!(
                    with_timeout(Duration::from_millis(100), sensor.initialize(&mut delay)).await,
                    Ok(Ok(_))
                )
            } else {
                true
            };
            let reading = if ready {
                with_timeout(Duration::from_millis(100), sensor.measure(&mut delay))
                    .await
                    .ok()
                    .and_then(Result::ok)
            } else {
                None
            };
            let chip = sensor.chip();
            last_sample = Some(Instant::now());
            update(|s| {
                s.chip = chip;
                s.reading = reading;
                s.sampled_at_ms = Instant::now().as_millis();
                s.sensor_error = reading.is_none();
                if reading.is_some() {
                    s.samples = s.samples.saturating_add(1)
                }
            });
            if reading.is_none() {
                sensor = Sensor::new(sensor.into_inner(), address);
            }
            let s = critical_section::with(|cs| *STATE.borrow(cs).borrow());
            prepared = s
                .reading
                .and_then(|r| crate::environment_show::prepare(r, s.samples).ok());
        }
        let digits = prepared
            .as_ref()
            .map(|p| p.render(last_sample.map_or(30_000, |at| at.elapsed().as_millis())))
            .unwrap_or([0x40; 4]);
        // Checked finite physical transport only; no resident computation Back.
        let write = [0x40];
        let frame = [0xc0, digits[0], digits[1], digits[2], digits[3]];
        let enable = [0x88 | config::BRIGHTNESS];
        pins.begin();
        let ack = conduit_tm1637_wire::transmit(
            &mut pins,
            &[
                conduit_tm1637_wire::Packet { bytes: &write },
                conduit_tm1637_wire::Packet { bytes: &frame },
                conduit_tm1637_wire::Packet { bytes: &enable },
            ],
            || false,
        )
        .is_ok();
        update(|s| {
            s.frames = s.frames.saturating_add(1);
            s.display_error = !ack;
            if ack {
                s.acks = s.acks.saturating_add(1)
            }
        });
        let (_, retained) = sensor.into_parts();
        sensor_state = retained;
        drop(bus);
        Timer::after(Duration::from_secs(2)).await;
    }
}
