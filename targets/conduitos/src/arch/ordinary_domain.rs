//! The separately compiled text implementation runs outside privileged Root.
use super::{
    domain_memory::{AddressSpace, enable_no_execute},
    domain_transition,
};
use crate::{
    domain_image::DomainImage,
    protected_region::{DomainBackend, DomainCost, DomainFault, DomainRefusal, DomainReturn},
};
#[path = "../../domain/frame.rs"]
mod frame;
pub(super) use frame::{TEXT_CAPACITY, TextFrame};

#[cfg(target_arch = "x86_64")]
const IMAGE_MACHINE: u16 = 62;
#[cfg(target_arch = "x86")]
const IMAGE_MACHINE: u16 = 3;

#[cfg(target_arch = "aarch64")]
const IMAGE_MACHINE: u16 = 183;
#[cfg(target_arch = "riscv64")]
const IMAGE_MACHINE: u16 = 243;

#[cfg(target_arch = "loongarch64")]
const IMAGE_MACHINE: u16 = 258;

const IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/domain.elf"));

pub struct TextDomain {
    space: AddressSpace,
    cost: DomainCost,
    quarantined: bool,
    keymap_initialized: bool,
    editor_initialized: bool,
    #[cfg(feature = "ordinary-domain-proof")]
    gate_probe: (u32, u64),
}

impl TextDomain {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub const TICK_UNIT: &'static str = "tsc";
    #[cfg(target_arch = "aarch64")]
    pub const TICK_UNIT: &'static str = "cntvct";
    #[cfg(target_arch = "riscv64")]
    pub const TICK_UNIT: &'static str = "time";
    #[cfg(target_arch = "loongarch64")]
    pub const TICK_UNIT: &'static str = "rdtime";
    pub const RESERVED_BYTES: u32 = AddressSpace::RESERVED_BYTES;
    pub fn ticks() -> u64 {
        super::domain_ticks()
    }
    pub fn preparation_cost(&mut self, started: u64, root_metadata_bytes: u32) {
        self.cost.setup_ticks = Self::ticks().saturating_sub(started);
        self.cost.root_metadata_bytes = root_metadata_bytes;
    }
    pub fn install() -> Result<Self, DomainRefusal> {
        let started = super::domain_ticks();
        enable_no_execute()?;
        let image =
            DomainImage::parse(IMAGE, IMAGE_MACHINE).map_err(|_| DomainRefusal::InvalidMemory)?;
        let space = AddressSpace::install(&image)?;
        Ok(Self {
            space,
            cost: DomainCost {
                reserved_bytes: Self::RESERVED_BYTES,
                setup_copied_bytes: image
                    .segments
                    .iter()
                    .map(|segment| segment.bytes.len() as u64)
                    .sum(),
                setup_ticks: super::domain_ticks().saturating_sub(started),
                ..DomainCost::default()
            },
            quarantined: false,
            keymap_initialized: false,
            editor_initialized: false,
            #[cfg(feature = "ordinary-domain-proof")]
            gate_probe: (0, 0),
        })
    }
    pub fn input(&mut self, input: &[u8]) -> Result<(), DomainRefusal> {
        if input.len() > TEXT_CAPACITY || self.quarantined {
            return Err(DomainRefusal::InvalidMemory);
        }
        let frame = self.space.frame();
        frame.input.fill(0);
        frame.output.fill(0);
        frame.intermediate.fill(0);
        frame.intermediate_length = 0;
        frame.input[..input.len()].copy_from_slice(input);
        frame.input_length = input.len() as u32;
        frame.output_length = 0;
        frame.capacity = TEXT_CAPACITY as u32;
        frame.status = 3;
        frame.probe = 0;
        frame.target = 0;
        frame.command = 0;
        frame.operation = 0;
        frame.work_units = 0;
        frame.capability = 0;
        self.cost.copied_bytes += input.len() as u64;
        self.cost.shared_peak_bytes = self.cost.shared_peak_bytes.max(input.len() as u32);
        Ok(())
    }
    pub fn output(&mut self, output: &mut [u8]) -> Result<usize, DomainRefusal> {
        let frame = self.space.frame();
        let length = frame.output_length as usize;
        if self.quarantined || frame.status != 0 || length > TEXT_CAPACITY || length > output.len()
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        output[..length].copy_from_slice(&frame.output[..length]);
        self.cost.copied_bytes += length as u64;
        self.cost.shared_peak_bytes = self
            .cost
            .shared_peak_bytes
            .max(frame.input_length.min(TEXT_CAPACITY as u32) + length as u32);
        Ok(length)
    }
    pub fn status(&mut self) -> u32 {
        self.space.frame().status
    }
    pub fn initialize_keymap(&mut self) -> Result<(), DomainRefusal> {
        self.input(&[])?;
        self.space.frame().command = 3;
        self.keymap_initialized = false;
        self.editor_initialized = false;
        Ok(())
    }
    pub fn initialize_editor(&mut self, maximum: u64) -> Result<(), DomainRefusal> {
        self.input(&maximum.to_le_bytes())?;
        self.space.frame().command = 6;
        self.keymap_initialized = false;
        self.editor_initialized = false;
        Ok(())
    }
    pub fn keymap_edit_chain_input(&mut self, input: &[u8]) -> Result<(), DomainRefusal> {
        if !self.editor_initialized {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        self.keymap_input(input)?;
        self.space.frame().command = 7;
        Ok(())
    }
    pub fn keymap_input(&mut self, input: &[u8]) -> Result<(), DomainRefusal> {
        if !self.keymap_initialized {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        self.input(input)?;
        self.space.frame().command = 4;
        Ok(())
    }
    pub fn keymap_chain_input(&mut self, input: &[u8]) -> Result<(), DomainRefusal> {
        self.keymap_input(input)?;
        self.space.frame().command = 5;
        Ok(())
    }
    pub fn intermediate(&mut self, output: &mut [u8; 4]) -> Result<usize, DomainRefusal> {
        let frame = self.space.frame();
        let length = frame.intermediate_length as usize;
        let edit_refused = frame.command == 7 && frame.status == 2 && frame.output_length == 0;
        if self.quarantined
            || !matches!(frame.command, 5 | 7)
            || (frame.status != 0 && !edit_refused)
            || length > 4
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        output[..length].copy_from_slice(&frame.intermediate[..length]);
        self.cost.copied_bytes += length as u64;
        self.cost.shared_peak_bytes = self.cost.shared_peak_bytes.max(
            frame.input_length.min(TEXT_CAPACITY as u32)
                + frame.output_length.min(TEXT_CAPACITY as u32)
                + length as u32,
        );
        Ok(length)
    }
    pub fn presentation(&mut self, input: &[u8], handle: u64) -> Result<(), DomainRefusal> {
        self.input(input)?;
        let frame = self.space.frame();
        frame.command = 1;
        frame.capability = handle;
        frame.operation = crate::domain_serial_scope::SERIAL_PRESENT_OPERATION;
        frame.work_units = 1;
        #[cfg(feature = "ordinary-domain-proof")]
        {
            frame.probe = self.gate_probe.0;
            frame.target = self.gate_probe.1;
        }
        Ok(())
    }
    pub fn effect_request(
        &mut self,
        output: &mut [u8; TEXT_CAPACITY],
    ) -> Result<(u64, u32, u32, usize), DomainRefusal> {
        let frame = self.space.frame();
        let length = frame.input_length as usize;
        if self.quarantined
            || frame.command != 1
            || frame.capacity != TEXT_CAPACITY as u32
            || length > TEXT_CAPACITY
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        output[..length].copy_from_slice(&frame.input[..length]);
        self.cost.copied_bytes += length as u64;
        Ok((frame.capability, frame.operation, frame.work_units, length))
    }
    pub fn effect_completed(&mut self) {
        let frame = self.space.frame();
        frame.command = 2;
        frame.status = 0;
    }
    #[cfg(feature = "ordinary-domain-proof")]
    pub fn probe(&mut self, command: u32, target: u64) {
        let frame = self.space.frame();
        frame.probe = command;
        frame.target = target;
    }
    #[cfg(feature = "ordinary-domain-proof")]
    pub fn private_frame_address(&mut self) -> u64 {
        self.space.frame() as *mut TextFrame as u64
    }
    #[cfg(feature = "ordinary-domain-proof")]
    pub fn configure_gate_probe(&mut self, probe: u32, target: u64) {
        self.gate_probe = (probe, target);
    }
}

impl DomainBackend for TextDomain {
    fn enter(&mut self, maximum_work: u32) -> Result<DomainReturn, DomainRefusal> {
        if self.quarantined || maximum_work == 0 {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        let result = domain_transition::enter(&self.space)?;
        self.cost.entries += 1;
        self.cost.address_space_switches += 2;
        self.cost.tlb_flushes += 2;
        if matches!(self.space.frame().command, 3 | 6)
            && self.space.frame().status == 0
            && result.origin == 0
            && result.value == 0
        {
            self.keymap_initialized = true;
            self.editor_initialized = self.space.frame().command == 6;
        }
        self.cost.scheduler_returns += 1;
        let (budget_irqs, source_irqs) = super::domain_budget::user_interrupts();
        #[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
        let interrupts = u64::from(budget_irqs) + u64::from(source_irqs);
        #[cfg(any(target_arch = "riscv64", target_arch = "loongarch64"))]
        let _ = budget_irqs;
        #[cfg(any(target_arch = "riscv64", target_arch = "loongarch64"))]
        let interrupts = u64::from(super::domain_budget::interrupt_entries());
        self.cost.interrupt_entries += interrupts;
        self.cost.source_timer_interrupts += u64::from(source_irqs);
        // Terminal IRQ return already contributes the ordinary Root return.
        self.cost.privilege_transitions +=
            2 + 2 * interrupts.saturating_sub(u64::from(result.origin == 2));
        #[cfg(feature = "ordinary-domain-proof")]
        {
            let mut sign = crate::sign_format::FixedText::new();
            use core::fmt::Write;
            let _ = writeln!(
                sign,
                "CONDUIT_DOMAIN_RETURN {} {}",
                result.origin, result.value
            );
            super::early_write(sign.as_bytes());
        }
        Ok(match (result.origin, result.value) {
            (0, 0..=2) => {
                self.cost.gate_transitions += 1;
                DomainReturn::Yielded
            }
            (0, 0x200) => {
                self.cost.gate_transitions += 1;
                self.cost.base_gate_transitions += 1;
                DomainReturn::Gate
            }
            (2, 4) => {
                self.cost.preemptions += 1;
                DomainReturn::Fault(DomainFault::WorkExhausted)
            }
            (1, 0x10d) => DomainReturn::Fault(DomainFault::PrivilegedOperation),
            (1, 0x10e) => DomainReturn::Fault(DomainFault::Memory),
            (1, _) => DomainReturn::Fault(DomainFault::InvalidInstruction),
            _ => DomainReturn::Fault(DomainFault::InvalidGate),
        })
    }
    fn quarantine(&mut self) {
        if !self.quarantined {
            let started = super::domain_ticks();
            self.cost.teardown_zeroed_bytes = self.space.quarantine();
            self.cost.teardown_ticks = super::domain_ticks().saturating_sub(started);
        }
        self.quarantined = true;
    }
    fn cost(&self) -> DomainCost {
        self.cost
    }
}
