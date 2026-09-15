//! Discovered local-APIC interrupt controller and bounded one-shot timer.

use core::{
    arch::{asm, x86_64::__cpuid},
    sync::atomic::{AtomicBool, Ordering},
};

use super::acpi::InterruptTopology;

const LOCAL_ID: u64 = 0x20;
const LOCAL_TASK_PRIORITY: u64 = 0x80;
const LOCAL_EOI: u64 = 0xb0;
const LOCAL_SPURIOUS_VECTOR: u64 = 0xf0;
const LOCAL_TIMER: u64 = 0x320;
const LOCAL_LINT_ZERO: u64 = 0x350;
const LOCAL_LINT_ONE: u64 = 0x360;
const LOCAL_TIMER_INITIAL_COUNT: u64 = 0x380;
const LOCAL_TIMER_DIVIDE: u64 = 0x3e0;
const SOFTWARE_ENABLE: u32 = 1 << 8;
const MASKED: u32 = 1 << 16;
const TIMER_VECTOR: u32 = 0x20;
const DIVIDE_BY_SIXTEEN: u32 = 0b0011;
const ONE_SHOT_COUNT: u32 = 100_000;
static LEGACY: AtomicBool = AtomicBool::new(false);

const IA32_APIC_BASE: u32 = 0x1b;
const APIC_ENABLED: u64 = 1 << 11;
const X2APIC_ENABLED: u64 = 1 << 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerRefusal {
    Address,
    Identity,
}

impl ControllerRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Address => "interrupt-controller-address-invalid",
            Self::Identity => "interrupt-controller-identity-mismatch",
        }
    }
}

pub(super) fn initialize(
    _hhdm: u64,
    topology: InterruptTopology,
    _image_virtual_to_physical: fn(u64) -> Option<u64>,
) -> Result<(), ControllerRefusal> {
    if __cpuid(1).ecx & (1 << 21) == 0 {
        LEGACY.store(true, Ordering::Release);
        return Ok(());
    }
    let apic_base = read_msr(IA32_APIC_BASE);
    if topology.local_apic_address & 0xfff != 0 {
        return Err(ControllerRefusal::Address);
    }
    write_msr(IA32_APIC_BASE, apic_base | APIC_ENABLED | X2APIC_ENABLED);
    let actual_id = local_read(LOCAL_ID);
    if actual_id != u32::from(topology.local_apic_id) {
        return Err(ControllerRefusal::Identity);
    }
    local_write(LOCAL_TASK_PRIORITY, 0);
    local_write(LOCAL_SPURIOUS_VECTOR, SOFTWARE_ENABLE | 0xff);
    local_write(LOCAL_LINT_ZERO, MASKED);
    local_write(LOCAL_LINT_ONE, MASKED);
    local_write(LOCAL_TIMER_INITIAL_COUNT, 0);
    local_write(LOCAL_TIMER, MASKED | TIMER_VECTOR);
    local_write(LOCAL_TIMER_DIVIDE, DIVIDE_BY_SIXTEEN);
    Ok(())
}

pub(super) fn arm_timer() {
    if LEGACY.load(Ordering::Acquire) {
        super::pit::arm_one_shot();
        super::pic::unmask_timer();
        return;
    }
    local_write(LOCAL_TIMER, TIMER_VECTOR);
    local_write(LOCAL_TIMER_INITIAL_COUNT, ONE_SHOT_COUNT);
}

pub(super) fn cancel_timer() {
    if LEGACY.load(Ordering::Acquire) {
        super::pic::mask_timer();
        return;
    }
    local_write(LOCAL_TIMER_INITIAL_COUNT, 0);
    local_write(LOCAL_TIMER, MASKED | TIMER_VECTOR);
}

pub(super) fn end_timer_interrupt() {
    if LEGACY.load(Ordering::Acquire) {
        super::pic::end_timer_interrupt();
        return;
    }
    local_write(LOCAL_EOI, 0);
}

fn local_read(offset: u64) -> u32 {
    read_msr(0x800 + offset as u32 / 16) as u32
}

fn local_write(offset: u64, value: u32) {
    write_msr(0x800 + offset as u32 / 16, u64::from(value));
}

fn read_msr(register: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe { asm!("rdmsr", in("ecx") register, out("eax") low, out("edx") high) };
    u64::from(low) | u64::from(high) << 32
}

fn write_msr(register: u32, value: u64) {
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") register,
            in("eax") value as u32,
            in("edx") (value >> 32) as u32,
        )
    };
}
