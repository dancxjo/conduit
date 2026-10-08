//! Bounded numeric timer-runtime commands and independently validated copy windows.
use super::*;
use crate::tour_timer_kernel::runtime::PreparedTimerGraph;
use conduit_kernel::{
    BoundedValueRef, HostCallId, NodeId, RequestId, ValueRef, scheduler::HostCallRequest,
};

pub(crate) struct TimerDomainObservation {
    pub status: u32,
    pub decisions: u32,
    pub signs: u16,
    pub pending: u16,
}
pub(crate) struct TimerDomainRequest {
    pub kind: u32,
    pub request: HostCallRequest,
    pub handle: u64,
    pub operation: u32,
    pub work_units: u32,
    pub payload: [u8; 20],
    pub length: usize,
}

impl TextDomain {
    pub(crate) fn initialize_timer(
        &mut self,
        graph: PreparedTimerGraph,
        timer: u64,
        count: u64,
    ) -> Result<(), DomainRefusal> {
        if timer == 0 || count == 0 || timer == count {
            return Err(DomainRefusal::WrongBinding);
        }
        let mut encoded = [0; TEXT_CAPACITY];
        let length = graph
            .encode(&mut encoded)
            .map_err(|_| DomainRefusal::InvalidMemory)?;
        self.input(&encoded[..length])?;
        let frame = self.space.frame();
        frame.command = 10;
        frame.timer_handle = timer;
        frame.count_handle = count;
        frame.timer_status = 3;
        self.timer_initialized = false;
        self.cost.copied_bytes += 16;
        self.cost.shared_peak_bytes = self.cost.shared_peak_bytes.max(length as u32 + 16);
        Ok(())
    }

    pub(crate) fn timer_initialized(&mut self) -> Result<(), DomainRefusal> {
        let frame = self.space.frame();
        if self.quarantined || frame.command != 10 || frame.status != 0 || frame.timer_status != 0 {
            return Err(DomainRefusal::InvalidMemory);
        }
        self.timer_initialized = true;
        Ok(())
    }

    pub(crate) fn timer_command(&mut self, command: u32) -> Result<(), DomainRefusal> {
        if !self.timer_initialized || !matches!(command, 11 | 13) {
            return Err(DomainRefusal::InvalidMemory);
        }
        self.input(&[])?;
        let frame = self.space.frame();
        frame.command = command;
        frame.timer_status = 3;
        Ok(())
    }

    pub(crate) fn timer_completion(
        &mut self,
        kind: u32,
        node: NodeId,
        request: RequestId,
    ) -> Result<(), DomainRefusal> {
        if !self.timer_initialized
            || !matches!(kind, 1 | 2)
            || node.0 >= 3
            || !(1..=2).contains(&request.0)
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        self.input(&[])?;
        let frame = self.space.frame();
        frame.command = 12;
        frame.timer_kind = kind;
        frame.timer_node = u32::from(node.0);
        frame.timer_request = request.0;
        frame.timer_status = 3;
        self.cost.copied_bytes += 12;
        self.cost.shared_peak_bytes = self.cost.shared_peak_bytes.max(12);
        Ok(())
    }

    pub(crate) fn timer_observation(&mut self) -> Result<TimerDomainObservation, DomainRefusal> {
        let frame = self.space.frame();
        if self.quarantined
            || !(10..=13).contains(&frame.command)
            || !matches!(frame.timer_status, 0..=2 | 0x200)
            || frame.timer_signs > 96
            || frame.timer_pending > 2
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        Ok(TimerDomainObservation {
            status: frame.timer_status,
            decisions: frame.timer_decisions,
            signs: frame.timer_signs as u16,
            pending: frame.timer_pending as u16,
        })
    }

    pub(crate) fn timer_request(&mut self) -> Result<TimerDomainRequest, DomainRefusal> {
        let frame = self.space.frame();
        let length = frame.output_length as usize;
        if self.quarantined
            || !self.timer_initialized
            || frame.command != 11
            || frame.timer_status != 0x200
            || frame.status != 0
            || frame.timer_node >= 3
            || !(1..=2).contains(&frame.timer_request)
            || frame.timer_slot >= 6
            || frame.timer_generation == 0
            || frame.timer_generation > u32::from(u16::MAX)
            || frame.timer_value_bytes != 8
            || frame.timer_admitted_bytes != 8
            || !matches!(frame.timer_kind, 1 | 2)
            || length == 0
            || length > 20
            || (frame.timer_kind == 1 && length != 8)
        {
            return Err(DomainRefusal::InvalidMemory);
        }
        let mut payload = [0; 20];
        payload[..length].copy_from_slice(&frame.output[..length]);
        let request = TimerDomainRequest {
            kind: frame.timer_kind,
            request: HostCallRequest {
                node: NodeId(frame.timer_node as u16),
                request: RequestId(frame.timer_request),
                call: HostCallId(0),
                input: BoundedValueRef {
                    value: ValueRef {
                        slot: frame.timer_slot as u16,
                        generation: frame.timer_generation as u16,
                        byte_len: 8,
                    },
                    admitted_bytes: 8,
                },
            },
            handle: frame.capability,
            operation: frame.operation,
            work_units: frame.work_units,
            payload,
            length,
        };
        self.cost.copied_bytes += length as u64 + 24;
        self.cost.shared_peak_bytes = self.cost.shared_peak_bytes.max(length as u32 + 24);
        Ok(request)
    }
}
