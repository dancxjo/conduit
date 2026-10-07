//! The separately compiled text implementation runs outside privileged Root.
use super::{
    domain_memory::{AddressSpace, enable_no_execute},
    domain_transition,
};
use crate::{
    domain_image::DomainImage,
    protected_region::{DomainBackend, DomainCost, DomainFault, DomainRefusal, DomainReturn},
};
#[path = "../../../domain/frame.rs"]
mod frame;
pub(super) use frame::{TEXT_CAPACITY, TextFrame};

const IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/domain.elf"));

pub struct TextDomain {
    space: AddressSpace,
    cost: DomainCost,
    quarantined: bool,
}

impl TextDomain {
    pub const RESERVED_BYTES: u32 = 4 * 4096 + 65536 + 4096 + 2 * 16384;
    pub fn install() -> Result<Self, DomainRefusal> {
        enable_no_execute()?;
        let image = DomainImage::parse(IMAGE, 62).map_err(|_| DomainRefusal::InvalidMemory)?;
        let space = AddressSpace::install(&image)?;
        Ok(Self {
            space,
            cost: DomainCost {
                reserved_bytes: Self::RESERVED_BYTES,
                copied_bytes: image
                    .segments
                    .iter()
                    .map(|segment| segment.bytes.len() as u64)
                    .sum(),
                ..DomainCost::default()
            },
            quarantined: false,
        })
    }
    pub fn input(&mut self, input: &[u8]) -> Result<(), DomainRefusal> {
        if input.len() > TEXT_CAPACITY || self.quarantined {
            return Err(DomainRefusal::InvalidMemory);
        }
        let frame = self.space.frame();
        frame.input.fill(0);
        frame.output.fill(0);
        frame.input[..input.len()].copy_from_slice(input);
        frame.input_length = input.len() as u32;
        frame.output_length = 0;
        frame.capacity = TEXT_CAPACITY as u32;
        frame.status = 3;
        frame.probe = 0;
        frame.target = 0;
        self.cost.copied_bytes += input.len() as u64;
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
        Ok(length)
    }
    pub fn status(&mut self) -> u32 {
        self.space.frame().status
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
}

impl DomainBackend for TextDomain {
    fn enter(&mut self, maximum_work: u32) -> Result<DomainReturn, DomainRefusal> {
        if self.quarantined || maximum_work == 0 {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        self.cost.entries += 1;
        self.cost.address_space_switches += 2;
        let result = domain_transition::enter(&self.space)?;
        self.cost.scheduler_returns += 1;
        Ok(match result {
            0..=2 => {
                self.cost.gate_transitions += 1;
                DomainReturn::Yielded
            }
            4 => {
                self.cost.preemptions += 1;
                DomainReturn::Fault(DomainFault::WorkExhausted)
            }
            0x10d => DomainReturn::Fault(DomainFault::PrivilegedOperation),
            0x10e => DomainReturn::Fault(DomainFault::Memory),
            _ => DomainReturn::Fault(DomainFault::InvalidInstruction),
        })
    }
    fn quarantine(&mut self) {
        self.quarantined = true;
    }
    fn cost(&self) -> DomainCost {
        self.cost
    }
}
