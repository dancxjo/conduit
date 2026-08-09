//! Bounded reboot record for otherwise-unobservable Wi-Fi bootstrap panics.

use core::panic::PanicInfo;
use embassy_rp::{
    peripherals::WATCHDOG,
    watchdog::{ResetReason, Watchdog},
    Peri,
};

const RECORD_MAGIC: u32 = 0x434e_4400;
const RECORD_MAGIC_MASK: u32 = 0xffff_ff00;

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum PanicPhase {
    RadioDriverStartup = 1,
    RadioInitialization = 2,
    Unclassified = 255,
}

pub struct PanicRecord {
    phase: PanicPhase,
}

impl PanicRecord {
    pub const fn code(&self) -> &'static str {
        match self.phase {
            PanicPhase::RadioDriverStartup => "radio-driver-startup-panic",
            PanicPhase::RadioInitialization => "radio-initialization-panic",
            PanicPhase::Unclassified => "firmware-panic",
        }
    }
}

pub fn set_phase(phase: PanicPhase) {
    let watchdog = rp_pac::WATCHDOG;
    watchdog
        .scratch0()
        .write(|value| *value = RECORD_MAGIC | phase as u32);
}

pub fn clear() {
    let watchdog = rp_pac::WATCHDOG;
    watchdog.scratch0().write(|value| *value = 0);
}

pub fn take(watchdog_peripheral: Peri<'static, WATCHDOG>) -> Option<PanicRecord> {
    let mut watchdog = Watchdog::new(watchdog_peripheral);
    let forced_reset = watchdog.reset_reason() == Some(ResetReason::Forced);
    let record = watchdog.get_scratch(0);
    watchdog.set_scratch(0, 0);
    if !forced_reset || record & RECORD_MAGIC_MASK != RECORD_MAGIC {
        return None;
    }
    Some(PanicRecord {
        phase: match record & !RECORD_MAGIC_MASK {
            1 => PanicPhase::RadioDriverStartup,
            2 => PanicPhase::RadioInitialization,
            _ => PanicPhase::Unclassified,
        },
    })
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    let watchdog = rp_pac::WATCHDOG;
    if watchdog.scratch0().read() & RECORD_MAGIC_MASK != RECORD_MAGIC {
        set_phase(PanicPhase::Unclassified);
    }
    rp_pac::PSM
        .wdsel()
        .write_value(rp_pac::psm::regs::Wdsel(0x0001_fffc));
    watchdog.ctrl().modify(|value| value.set_trigger(true));
    loop {
        cortex_m::asm::wfi();
    }
}
