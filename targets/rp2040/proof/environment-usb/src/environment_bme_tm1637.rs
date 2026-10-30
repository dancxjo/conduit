//! Checked mixed-bus hardware composition; not yet an admitted Body executor.
use crate::config;
use conduit_bosch_env::{Address, Humidity, Reading, Sensor};
use conduit_rp2040_tm1637::PicoTm1637;
use conduit_tm1637_wire::{transmit, OpenDrain, Packet};
use core::{cell::RefCell, convert::Infallible, fmt::Write};
use critical_section::Mutex;
use embassy_rp::{
    i2c::{self, I2c},
    Peri,
};
use embassy_time::{with_timeout, Delay, Duration, Instant, Timer};
#[derive(Clone, Copy)]
struct Snapshot {
    samples: u64,
    attempts: u64,
    frames: u64,
    acks: u64,
    sampled_at_ms: u64,
    reading: Option<Reading>,
    sensor_error: bool,
    display_error: bool,
}
static STATE: Mutex<RefCell<Snapshot>> = Mutex::new(RefCell::new(Snapshot {
    samples: 0,
    attempts: 0,
    frames: 0,
    acks: 0,
    sampled_at_ms: 0,
    reading: None,
    sensor_error: false,
    display_error: false,
}));
fn snapshot() -> Snapshot {
    critical_section::with(|cs| *STATE.borrow(cs).borrow())
}
fn update(f: impl FnOnce(&mut Snapshot)) {
    critical_section::with(|cs| f(&mut STATE.borrow(cs).borrow_mut()))
}
pub fn status() -> Result<heapless::String<1024>, ()> {
    let s = snapshot();
    let mut out = heapless::String::new();
    let (_, peak) = crate::heap::usage();
    write!(&mut out,"{{\"role\":\"{}\",\"build\":\"{}\",\"configuration\":\"{}\",\"sensor_address\":{},\"clock_gpio\":{},\"data_gpio\":{},\"samples\":{},\"attempts\":{},\"frames\":{},\"acks\":{},\"sampled_at_boot_ms\":{},\"heap_peak\":{},\"sensor_error\":{},\"display_error\":{},\"body_member\":false",config::ROLE,crate::protocol::BUILD_ID,config::CONFIGURATION_ID,config::SENSOR_ADDRESS,config::DISPLAY_CLK_GPIO,config::DISPLAY_DIO_GPIO,s.samples,s.attempts,s.frames,s.acks,s.sampled_at_ms,peak,s.sensor_error,s.display_error).map_err(|_|())?;
    if let Some(r) = s.reading {
        write!(&mut out,",\"temperature_centi_celsius\":{},\"pressure_centi_pascal\":{},\"humidity_percent_q10\":",r.temperature_centi_celsius,r.pressure_centi_pascal).map_err(|_|())?;
        match r.humidity {
            Humidity::PercentQ10(h) => write!(&mut out, "{h}"),
            Humidity::Unavailable(_) => write!(&mut out, "null"),
        }
        .map_err(|_| ())?;
    }
    crate::flash_observation::append(&mut out).map_err(|_| ())?;
    out.push('}').map_err(|_| ())?;
    Ok(out)
}
struct Display(PicoTm1637<'static>);
impl OpenDrain for Display {
    type Error = Infallible;
    fn clock(&mut self, released: bool) -> Result<(), Infallible> {
        self.0.clock(released)
    }
    fn data(&mut self, released: bool) -> Result<(), Infallible> {
        self.0.data(released)
    }
    fn read_data(&mut self) -> Result<bool, Infallible> {
        self.0.read_data()
    }
    fn half_period(&mut self) -> Result<(), Infallible> {
        embassy_time::block_for(Duration::from_micros(u64::from(
            config::DISPLAY_HALF_PERIOD_US,
        )));
        Ok(())
    }
}
#[embassy_executor::task]
pub async fn run(
    bus: Peri<'static, config::SensorBus>,
    scl: Peri<'static, config::SensorScl>,
    sda: Peri<'static, config::SensorSda>,
    clock: Peri<'static, config::DisplayClk>,
    data: Peri<'static, config::DisplayDio>,
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
    let mut pins = PicoTm1637::new(clock, data);
    pins.set_internal_pullups(config::DISPLAY_WEAK_PULLUPS);
    let mut display = Display(pins);
    let mut delay = Delay;
    let mut last_sample = None;
    let mut prepared = None;
    loop {
        if last_sample.is_none_or(|at: Instant| at.elapsed() >= Duration::from_secs(30)) {
            update(|s| s.attempts = s.attempts.saturating_add(1));
            let ready = sensor.chip().is_some()
                || matches!(
                    with_timeout(Duration::from_millis(100), sensor.initialize(&mut delay)).await,
                    Ok(Ok(_))
                );
            let reading = if ready {
                with_timeout(Duration::from_millis(100), sensor.measure(&mut delay))
                    .await
                    .ok()
                    .and_then(Result::ok)
            } else {
                None
            };
            let at = Instant::now();
            last_sample = Some(at);
            update(|s| {
                s.reading = reading;
                s.sampled_at_ms = at.as_millis();
                s.sensor_error = reading.is_none();
                if reading.is_some() {
                    s.samples = s.samples.saturating_add(1);
                }
            });
            if reading.is_none() {
                sensor = Sensor::new(sensor.into_inner(), address);
            }
            let s = snapshot();
            prepared = s
                .reading
                .and_then(|r| crate::environment_show::prepare(r, s.samples).ok());
        }
        let digits = prepared
            .as_ref()
            .map(|show| show.render(last_sample.map_or(30_000, |at| at.elapsed().as_millis())))
            .unwrap_or([0x40; 4]);
        let data = [0xc0, digits[0], digits[1], digits[2], digits[3], 0, 0];
        let brightness = [0x88 | config::BRIGHTNESS];
        let ack = transmit(
            &mut display,
            &[
                Packet { bytes: &[0x40] },
                Packet { bytes: &data },
                Packet { bytes: &brightness },
            ],
            || false,
        )
        .is_ok();
        update(|s| {
            s.frames = s.frames.saturating_add(1);
            s.display_error = !ack;
            if ack {
                s.acks = s.acks.saturating_add(1);
            }
        });
        Timer::after_secs(2).await;
    }
}
