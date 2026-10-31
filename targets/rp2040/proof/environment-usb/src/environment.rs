//! Autonomous diagnostic quantities/Face/Look; no installed Body or Plan claim.
use crate::config;
use conduit_bosch_env::{Address, Humidity, Reading, Sensor};
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
    let chip_id = match s.chip {
        Some(conduit_bosch_env::Chip::Bme280) => 96,
        Some(conduit_bosch_env::Chip::Bmp280) => 88,
        None => 0,
    };
    write!(&mut out,"{{\"role\":\"{}\",\"build\":\"{}\",\"configuration\":\"{}\",\"sensor_address\":{},\"display_address\":{},\"samples\":{},\"attempts\":{},\"frames\":{},\"acks\":{},\"heap_peak\":{},\"sensor_error\":{},\"display_error\":{},\"body_member\":false",config::ROLE,crate::protocol::BUILD_ID,config::CONFIGURATION_ID,config::SENSOR_ADDRESS,config::DISPLAY_ADDRESS,s.samples,s.attempts,s.frames,s.acks,peak,s.sensor_error,s.display_error).map_err(|_|())?;
    write!(&mut out, ",\"chip_id\":{chip_id}").map_err(|_| ())?;
    write!(&mut out, ",\"sampled_at_boot_ms\":{}", s.sampled_at_ms).map_err(|_| ())?;
    if let Some(r) = s.reading {
        write!(&mut out,",\"temperature_centi_celsius\":{},\"pressure_centi_pascal\":{},\"humidity_percent_q10\":",r.temperature_centi_celsius,r.pressure_centi_pascal).map_err(|_|())?;
        match r.humidity {
            Humidity::PercentQ10(h) => write!(&mut out, "{h}"),
            Humidity::Unavailable(_) => write!(&mut out, "null"),
        }
        .map_err(|_| ())?;
    }
    #[cfg(feature = "zenoh-evaluation")]
    crate::zenoh_evaluation::status(&mut out).map_err(|_| ())?;
    crate::flash_observation::append(&mut out).map_err(|_| ())?;
    #[cfg(feature = "credential-store")]
    crate::credential_store::append(&mut out).map_err(|_| ())?;
    out.push('}').map_err(|_| ())?;
    Ok(out)
}
#[embassy_executor::task]
pub async fn run(
    bus: Peri<'static, config::Bus>,
    scl: Peri<'static, config::Scl>,
    sda: Peri<'static, config::Sda>,
) {
    let mut bus_config = i2c::Config::default();
    bus_config.frequency = config::FREQUENCY;
    let address = if config::SENSOR_ADDRESS == 0x76 {
        Address::Primary
    } else {
        Address::Secondary
    };
    let mut sensor = Sensor::new(
        I2c::new_async(bus, scl, sda, config::SensorIrqs, bus_config),
        address,
    );
    let display = conduit_ht16k33::Address::new(config::DISPLAY_ADDRESS).unwrap();
    let display_config = conduit_ht16k33::Configuration {
        brightness: conduit_ht16k33::Brightness::new(config::BRIGHTNESS).unwrap(),
        blink: conduit_ht16k33::Blink::Off,
        enabled: true,
    };
    let mut initialized = false;
    let mut last_sample = None;
    let mut prepared = None;
    #[cfg(feature = "credential-store")]
    let mut credentials = crate::credential_store::take_owner();
    let mut delay = Delay;
    loop {
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
        if !initialized {
            initialized = matches!(
                with_timeout(
                    Duration::from_millis(100),
                    conduit_ht16k33::initialize(sensor.bus_mut(), display, display_config)
                )
                .await,
                Ok(Ok(()))
            );
        }
        let frame = prepared
            .as_ref()
            .and_then(|show| {
                crate::environment_show::frame(
                    show,
                    last_sample.map_or(30_000, |at| at.elapsed().as_millis()),
                )
                .ok()
            })
            .unwrap_or_else(|| {
                crate::config::display_profile().encode(
                    [conduit_ht16k33::Glyph::from_char('-').unwrap(); 4],
                    [false; 4],
                )
            });
        let ack = initialized
            && matches!(
                with_timeout(
                    Duration::from_millis(100),
                    conduit_ht16k33::write_frame(sensor.bus_mut(), display, &frame)
                )
                .await,
                Ok(Ok(()))
            );
        update(|s| {
            s.frames = s.frames.saturating_add(1);
            s.display_error = !ack;
            if ack {
                s.acks = s.acks.saturating_add(1)
            }
        });
        if !ack {
            initialized = false;
        }
        #[cfg(feature = "credential-store")]
        if let Some(owner) = credentials.as_mut() {
            owner.wait().await;
            continue;
        }
        Timer::after(Duration::from_secs(2)).await;
    }
}
