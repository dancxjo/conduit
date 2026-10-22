#![no_std]
//! PIO/DMA realization of bounded pulse geometry, independent of a peripheral.
use conduit_pulse_capture::{
    CheckedPulseRead, PulseReadCapture, PulseReadDisposition, PulseReadLimits, PulseReadRefusal,
};
use embassy_rp::{
    clocks::clk_sys_freq,
    dma::Channel,
    gpio::{Level, Pull},
    pio::{
        Common, Config, Direction, Instance, LoadedProgram, Pin, PioPin, ShiftDirection,
        StateMachine,
    },
    Peri,
};
use embassy_time::{with_timeout, Duration};
use fixed::types::U24F8;

pub const LIMITS: PulseReadLimits = PulseReadLimits {
    maximum_hold_low_us: 50_000,
    maximum_skipped_pulses: 4,
    minimum_sample_offset_us: 10,
    maximum_sample_offset_us: 200,
    maximum_octets: 8,
    maximum_capture_timeout_us: 50_000,
};

pub struct PulseProgram<'d, PIO: Instance>(LoadedProgram<'d, PIO>);
impl<'d, PIO: Instance> PulseProgram<'d, PIO> {
    pub fn load(common: &mut Common<'d, PIO>) -> Self {
        let program = pio::pio_asm!(
            r#"
            pull block
            mov x, osr
            set pindirs, 1
        drive_low:
            jmp x-- drive_low
            set pindirs, 0
            pull block
            mov x, osr
            pull block
            mov y, osr
            wait 1 pin 0
            wait 0 pin 0
        skip_check:
            jmp x-- skip_pulse
            jmp sample
        skip_pulse:
            wait 1 pin 0
            wait 0 pin 0
            jmp skip_check
        sample:
            wait 1 pin 0
            nop [31]
            nop [6]
            in pins, 1
            wait 0 pin 0
            jmp y-- sample
        retired:
            jmp retired
        "#
        );
        Self(common.load_program(&program.program))
    }
}

/// Construction consumes the exact selected GPIO, state machine and DMA channel.
/// Host installation must separately bind this ownership to its Plan and Play.
pub struct PicoPulseCapture<'d, PIO: Instance, const SM: usize> {
    sm: StateMachine<'d, PIO, SM>,
    pin: Pin<'d, PIO>,
    dma: Channel<'d>,
    failed: bool,
    program: PulseProgram<'d, PIO>,
}
impl<'d, PIO: Instance, const SM: usize> PicoPulseCapture<'d, PIO, SM> {
    pub fn new(
        common: &mut Common<'d, PIO>,
        mut sm: StateMachine<'d, PIO, SM>,
        dma: Channel<'d>,
        pin: Peri<'d, impl PioPin + 'd>,
        program: PulseProgram<'d, PIO>,
    ) -> Self {
        let mut pin = common.make_pio_pin(pin);
        pin.set_pull(Pull::Up);
        sm.set_enable(false);
        sm.set_pins(Level::Low, &[&pin]);
        sm.set_pin_dirs(Direction::In, &[&pin]);
        Self {
            sm,
            pin,
            dma,
            program,
            failed: false,
        }
    }

    pub async fn read(
        &mut self,
        checked: CheckedPulseRead,
    ) -> Result<PulseReadCapture, PulseReadRefusal> {
        if self.failed {
            return Ok(PulseReadCapture::from_complete_octets(
                PulseReadDisposition::ProviderFailed,
                &[],
            )
            .unwrap());
        }
        // Recheck the selected realization's limits before changing any pin.
        let request = CheckedPulseRead::admit(checked.request(), LIMITS)?.request();
        let divider_bits =
            u64::from(clk_sys_freq()) * u64::from(request.sample_after_rise_us) * 256 / 40_000_000;
        let divider_bits =
            u32::try_from(divider_bits).map_err(|_| PulseReadRefusal::SampleOffset)?;
        if !(256..=0x1000000).contains(&divider_bits) {
            return Err(PulseReadRefusal::SampleOffset);
        }
        let mut config = Config::default();
        config.use_program(&self.program.0, &[]);
        config.set_in_pins(&[&self.pin]);
        config.set_set_pins(&[&self.pin]);
        config.clock_divider = U24F8::from_bits(divider_bits);
        config.shift_in.auto_fill = true;
        config.shift_in.threshold = 8;
        config.shift_in.direction = ShiftDirection::Left;
        let low_cycles =
            u64::from(request.hold_low_us) * 40 / u64::from(request.sample_after_rise_us);
        if low_cycles < 2 {
            return Err(PulseReadRefusal::DriveExtent);
        }
        self.sm.set_enable(false);
        self.sm.set_config(&config);
        self.sm.restart();
        self.sm.clear_fifos();
        self.sm.set_pins(Level::Low, &[&self.pin]);
        self.sm.set_pin_dirs(Direction::In, &[&self.pin]);
        let mut stop = ReleaseOnDrop {
            sm: &mut self.sm,
            pin: &self.pin,
        };
        assert!(stop
            .sm
            .tx()
            .try_push(u32::try_from(low_cycles - 1).unwrap()));
        assert!(stop.sm.tx().try_push(u32::from(request.skip_high_pulses)));
        assert!(stop.sm.tx().try_push(u32::from(request.octets) * 8 - 1));
        let mut words = [0_u32; 8];
        stop.sm.clkdiv_restart();
        stop.sm.set_enable(true);
        let complete = with_timeout(
            Duration::from_micros(checked.deadline_span_us()),
            stop.sm.rx().dma_pull(
                &mut self.dma,
                &mut words[..usize::from(request.octets)],
                false,
            ),
        )
        .await
        .is_ok();
        stop.stop();
        // The DMA future has completed or been dropped/aborted before observing
        // the remaining count and destination. Partial ISR bits stay private.
        let remaining = embassy_rp::pac::DMA
            .ch(usize::from(self.dma.number()))
            .trans_count()
            .read();
        let status = embassy_rp::pac::DMA
            .ch(usize::from(self.dma.number()))
            .ctrl_trig()
            .read();
        let disposition = if status.read_error() || status.write_error() {
            PulseReadDisposition::ProviderFailed
        } else if complete {
            PulseReadDisposition::Complete
        } else {
            PulseReadDisposition::TimedOut
        };
        self.failed = disposition == PulseReadDisposition::ProviderFailed;
        let count = usize::from(request.octets).saturating_sub(remaining as usize);
        let mut wire = [0; 8];
        for (target, word) in wire.iter_mut().zip(words).take(count) {
            *target = word as u8;
        }
        Ok(PulseReadCapture::from_complete_octets(disposition, &wire[..count]).unwrap())
    }
}

struct ReleaseOnDrop<'a, 'd, PIO: Instance, const SM: usize> {
    sm: &'a mut StateMachine<'d, PIO, SM>,
    pin: &'a Pin<'d, PIO>,
}
impl<PIO: Instance, const SM: usize> ReleaseOnDrop<'_, '_, PIO, SM> {
    fn stop(&mut self) {
        self.sm.set_enable(false);
        self.sm.set_pin_dirs(Direction::In, &[self.pin]);
    }
}
impl<PIO: Instance, const SM: usize> Drop for ReleaseOnDrop<'_, '_, PIO, SM> {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(feature = "host-call")]
impl<PIO: Instance, const SM: usize> conduit_pulse_capture::owner::PulseReadProvider
    for PicoPulseCapture<'_, PIO, SM>
{
    async fn read(
        &mut self,
        request: CheckedPulseRead,
    ) -> Result<PulseReadCapture, PulseReadRefusal> {
        PicoPulseCapture::read(self, request).await
    }
}
