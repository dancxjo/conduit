//! Emulator fixture: a firmware-era periodic IRQ is pending at machine takeover.
//! This is not a product initialization path or physical timing evidence.

use conduit_kernel::scheduler::SchedulerStatus;
use conduitos::{
    arch,
    cooperative_timer_lane::AdmittedLane,
    machine::{BaseError, TimerBase},
};

const MAXIMUM_POLLS: u32 = 100_000;

pub(super) fn prove() -> Result<(), &'static str> {
    arch::disable_interrupts();
    unsafe {
        // Recreate the inherited periodic source with every unrelated line
        // masked. No guest interrupt can dispatch before Conduit's IDT exists.
        write(0x21, 0xfe);
        write(0xa1, 0xff);
        write(0x43, 0x34);
        write(0x40, 1193_u16 as u8);
        write(0x40, (1193_u16 >> 8) as u8);
    }
    let mut inherited = false;
    for _ in 0..MAXIMUM_POLLS {
        unsafe { write(0x20, 0x0a) };
        if unsafe { read(0x20) } & 1 != 0 {
            inherited = true;
            break;
        }
    }
    if !inherited {
        return Err("fixture-inherited-pit-irq-absent");
    }
    arch::present(b"CONDUIT_IA32_TIMER_TAKEOVER inherited-periodic-irq0-pending\n");
    arch::initialize_machine();
    let mut timer = arch::Timer::new();
    arch::enable_interrupts();
    for _ in 0..MAXIMUM_POLLS {
        match timer.take_wake() {
            Ok(None) => {}
            Ok(Some(_)) => return Err("fixture-unadmitted-timer-completed"),
            Err(_) => {
                arch::disable_interrupts();
                return Err("timer-base-failed");
            }
        }
    }
    arch::disable_interrupts();
    arch::present(b"CONDUIT_IA32_TIMER_TAKEOVER unarmed-timer-clean\n");
    prove_cancellation(&mut timer)?;
    Ok(())
}

fn prove_cancellation(timer: &mut arch::Timer) -> Result<(), &'static str> {
    let mut lane = AdmittedLane::new().map_err(|_| "fixture-lane-unavailable")?;
    if !matches!(lane.step(), Ok(SchedulerStatus::Progress { .. })) {
        return Err("fixture-timer-request-absent");
    }
    let interest = lane
        .take_timer_interest()
        .map_err(|_| "fixture-timer-interest-absent")?;
    let old = timer.arm(interest).map_err(|_| "fixture-first-arm-failed")?;
    if timer.cancel(old) != Ok(interest) {
        return Err("fixture-current-cancel-failed");
    }
    let current = timer.arm(interest).map_err(|_| "fixture-rearm-failed")?;
    if current == old || timer.cancel(old) != Err(BaseError::StaleWake) {
        return Err("fixture-stale-cancel-not-refused");
    }
    arch::enable_interrupts();
    let mut wake = None;
    for _ in 0..MAXIMUM_POLLS {
        wake = timer.take_wake().map_err(|_| "fixture-rearmed-wake-failed")?;
        if wake.is_some() {
            break;
        }
    }
    arch::disable_interrupts();
    if wake != Some(interest) || timer.wake_count() != 1 {
        return Err("fixture-rearmed-exact-wake-absent");
    }
    lane.complete_timer(interest)
        .map_err(|_| "fixture-lane-completion-failed")?;
    if !matches!(lane.step(), Ok(SchedulerStatus::Drained)) || lane.pending() != 0 {
        return Err("fixture-lane-not-drained");
    }
    arch::present(b"CONDUIT_IA32_TIMER_TAKEOVER stale-cancel-preserved-exact-rearmed-wake\n");
    Ok(())
}

unsafe fn write(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value,
            options(nomem, nostack, preserves_flags));
    }
}

unsafe fn read(port: u16) -> u8 {
    let value;
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") value,
            options(nomem, nostack, preserves_flags));
    }
    value
}
