//! Continuous diagnostic DHT Source and common Face/Look, never admitted Play.
use crate::config;
use conduit_core::Quantity;
use conduit_environment_face::EnvironmentalReading;
use conduit_rp2040_pulse_capture::{PicoPulseCapture, PulseProgram};
use conduit_rp2040_tm1637::PicoTm1637;
use conduit_tm1637_wire::{transmit, Failure, OpenDrain, Packet};
use core::{cell::RefCell, fmt::Write};
use embassy_rp::{dma::Channel, pio::Pio, Peri};
use embassy_time::{Duration, Instant, Timer};
#[path = "dht_source.rs"]
mod source;
#[cfg(feature = "wifi")]
embassy_rp::bind_interrupts!(struct CaptureIrqs{PIO1_IRQ_0=>embassy_rp::pio::InterruptHandler<config::CapturePio>;});
#[cfg(not(feature = "wifi"))]
embassy_rp::bind_interrupts!(struct CaptureIrqs{PIO1_IRQ_0=>embassy_rp::pio::InterruptHandler<config::CapturePio>;DMA_IRQ_0=>embassy_rp::dma::InterruptHandler<config::CaptureDma>;});
// Only electrical timing changes: every transition and ACK read remains with
// the actual owned provider. No swap, driven-high pin or synthetic ACK exists.
struct ConfiguredDisplay(PicoTm1637<'static>);
impl OpenDrain for ConfiguredDisplay {
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
        embassy_time::block_for(Duration::from_micros(u64::from(
            config::DISPLAY_HALF_PERIOD_US,
        )));
        Ok(())
    }
}
impl ConfiguredDisplay {
    fn released_levels(&mut self) -> (bool, bool) {
        let _ = self.clock(true);
        let _ = self.data(true);
        embassy_time::block_for(Duration::from_micros(1000));
        self.0.levels()
    }
}
#[derive(Clone, Copy)]
struct Snapshot {
    attempts: u64,
    samples: u64,
    frames: u64,
    acks: u64,
    reading: Option<(Quantity, Quantity)>,
    error: &'static str,
    before: (bool, bool),
    after: (bool, bool),
    wire_error: &'static str,
    nack: Option<(u8, u8)>,
    digits: [u8; 4],
}
static STATE: critical_section::Mutex<RefCell<Snapshot>> =
    critical_section::Mutex::new(RefCell::new(Snapshot {
        attempts: 0,
        samples: 0,
        frames: 0,
        acks: 0,
        reading: None,
        error: "warming",
        before: (false, false),
        after: (false, false),
        wire_error: "not-attempted",
        nack: None,
        digits: [0; 4],
    }));
fn update(f: impl FnOnce(&mut Snapshot)) {
    critical_section::with(|cs| f(&mut STATE.borrow(cs).borrow_mut()))
}
pub fn status() -> Result<heapless::String<1536>, ()> {
    let s = critical_section::with(|cs| *STATE.borrow(cs).borrow());
    let (_, peak) = crate::heap::usage();
    let mut out = heapless::String::new();
    write!(&mut out,"{{\"role\":\"{}\",\"build\":\"{}\",\"configuration\":\"{}\",\"sensor\":\"dht11\",\"display\":\"tm1637\",\"body_member\":false,\"attempts\":{},\"samples\":{},\"frames\":{},\"acks\":{},\"heap_peak\":{},\"status\":\"{}\"",config::ROLE,crate::protocol::BUILD_ID,config::CONFIGURATION_ID,s.attempts,s.samples,s.frames,s.acks,peak,s.error).map_err(|_|())?;
    write!(&mut out,",\"clock_gpio\":{},\"data_gpio\":{},\"half_period_us\":{},\"weak_pullups\":{},\"settled_us\":1000,\"idle_clock_high\":{},\"idle_data_high\":{},\"after_clock_high\":{},\"after_data_high\":{},\"last_wire_error\":\"{}\",\"display_frame\":[{},{},{},{}]",config::DISPLAY_CLK_GPIO,config::DISPLAY_DIO_GPIO,config::DISPLAY_HALF_PERIOD_US,config::DISPLAY_WEAK_PULLUPS,s.before.0,s.before.1,s.after.0,s.after.1,s.wire_error,s.digits[0],s.digits[1],s.digits[2],s.digits[3]).map_err(|_|())?;
    if let Some((packet, byte)) = s.nack {
        write!(&mut out, ",\"nack_packet\":{packet},\"nack_byte\":{byte}").map_err(|_| ())?;
    }
    if let Some((t, h)) = s.reading {
        write!(&mut out,",\"temperature\":{{\"coefficient\":{},\"exponent\":{}}},\"humidity\":{{\"coefficient\":{},\"exponent\":{}}}",t.coefficient(),t.exponent(),h.coefficient(),h.exponent()).map_err(|_|())?;
    }
    out.push('}').map_err(|_| ())?;
    Ok(out)
}
#[embassy_executor::task]
pub async fn run(
    pio: Peri<'static, config::CapturePio>,
    dma: Peri<'static, config::CaptureDma>,
    data: Peri<'static, config::DhtPin>,
    clk: Peri<'static, config::DisplayClk>,
    dio: Peri<'static, config::DisplayDio>,
) {
    let mut pio = Pio::new(pio, CaptureIrqs);
    let program = PulseProgram::load(&mut pio.common);
    #[cfg(feature = "wifi")]
    let dma = Channel::new(dma, crate::wifi::CaptureDmaIrqs);
    #[cfg(not(feature = "wifi"))]
    let dma = Channel::new(dma, CaptureIrqs);
    let mut sensor = PicoPulseCapture::new(&mut pio.common, pio.sm0, dma, data, program);
    let mut provider = PicoTm1637::new(clk, dio);
    provider.set_internal_pullups(config::DISPLAY_WEAK_PULLUPS);
    let mut display = ConfiguredDisplay(provider);
    let mut decoder = source::SourceDecoder::prepare();
    decoder.check_adapter();
    let mut sampled = None;
    loop {
        if sampled.is_none_or(|at: Instant| at.elapsed() >= Duration::from_secs(30)) {
            update(|s| s.attempts = s.attempts.saturating_add(1));
            let reading = match sensor.read(decoder.pulse_request).await {
                Ok(c) if c.disposition == conduit_pulse_capture::PulseReadDisposition::Complete => {
                    decoder.evaluate(c.length, c.wire[..5].try_into().unwrap())
                }
                _ => Err("capture-unavailable"),
            };
            sampled = Some(Instant::now());
            update(|s| {
                s.error = reading.as_ref().err().copied().unwrap_or("ok");
                s.reading = reading.ok();
                if s.reading.is_some() {
                    s.samples = s.samples.saturating_add(1)
                }
            });
        }
        let s = critical_section::with(|cs| *STATE.borrow(cs).borrow());
        let digits = s
            .reading
            .and_then(|(t, h)| EnvironmentalReading::temperature_humidity(t, h).ok())
            .and_then(|r| {
                crate::environment_show::render_common(
                    r,
                    s.samples,
                    sampled.map_or(30_000, |at| at.elapsed().as_millis()),
                )
                .ok()
            })
            .unwrap_or([0x40; 4]);
        let bytes = [0xc0, digits[0], digits[1], digits[2], digits[3]];
        let before = display.released_levels();
        let result = transmit(
            &mut display,
            &[
                Packet { bytes: &[0x40] },
                Packet { bytes: &bytes },
                Packet {
                    bytes: &[0x88 | config::BRIGHTNESS],
                },
            ],
            || false,
        );
        let after = display.released_levels();
        let (wire_error, nack) = match &result {
            Ok(()) => ("acknowledged", None),
            Err(Failure::Nack { packet, byte }) => ("nack", Some((*packet, *byte))),
            Err(Failure::InvalidGeometry) => ("invalid-geometry", None),
            Err(Failure::Cancelled) => ("cancelled", None),
            Err(Failure::Provider(error)) => match *error {},
        };
        update(|s| {
            s.before = before;
            s.after = after;
            s.wire_error = wire_error;
            s.nack = nack;
            s.digits = digits;
            s.frames = s.frames.saturating_add(1);
            if result.is_ok() {
                s.acks = s.acks.saturating_add(1)
            }
        });
        Timer::after_secs(2).await;
    }
}
