//! Mechanical wiring of the descriptor Source driver and its reusable pass.
//! Descriptor bounds, continuation and terminal meaning belong to the plots.
use super::{pure_pipe::prepare_stage, *};
use crate::pure_protocol_play::PreparedPureProtocolPlay;
use conduit_core::*;

pub struct PreparedHidDescriptorPipe {
    driver: PreparedPureProtocolPlay,
    pass: PreparedPureProtocolPipe,
    input: PortId,
    iterate: PortId,
    work: PortId,
    output: PortId,
    incoming: ValuePayload,
    work_value: ValuePayload,
    result: ValuePayload,
    sequence: u64,
    remaining_submissions: u8,
    iteration_sequence: u64,
    working: bool,
    ready: bool,
    closed: bool,
    driver_complete: bool,
}
impl PreparedHidDescriptorPipe {
    pub fn prepare(host_id: HostId, boot_id: BootId) -> Result<Self, PurePipeRefusal> {
        use PurePipeRefusal as E;
        // Seven submitted descriptors, each bounded by eight four-item passes.
        // Retain all lifecycle evidence before Play, including typed fan-out.
        let storage = conduit_composite::KernelCompositeSignStorage {
            additional_local_items: 4096,
            additional_remote_items: 512,
        };
        let pass = PreparedPureProtocolPipe::prepare_with_storage(
            usb_hid_absolute_descriptor_stages_package().map_err(E::Source)?,
            "usb-hid-absolute-layout-pass",
            host_id.clone(),
            boot_id.clone(),
            storage,
        )?;
        let artifact = prepare_stage(
            usb_hid_absolute_descriptor_stages_package().map_err(E::Source)?,
            "usb-hid-absolute-layout-driver",
            host_id,
            boot_id,
        )?;
        let boundary = &artifact.artifact().definition().boundary;
        if boundary.input_fronts.len() != 2 || boundary.output_fronts.len() != 2 {
            return Err(E::Binding);
        }
        let input = |name: &str| {
            boundary
                .input_fronts
                .iter()
                .find(|f| f.external_port.port_id.as_str() == name)
                .map(|f| f.external_port.clone())
                .ok_or(E::Binding)
        };
        let output = |name: &str| {
            boundary
                .output_fronts
                .iter()
                .find(|f| f.external_port.port_id.as_str() == name)
                .map(|f| f.external_port.clone())
                .ok_or(E::Binding)
        };
        let input = input("input")?;
        let iterate = boundary
            .input_fronts
            .iter()
            .find(|f| f.external_port.port_id.as_str() == "iterate")
            .map(|f| f.external_port.port_id.clone())
            .ok_or(E::Binding)?;
        let work = output("work")?;
        let result = output("result")?;
        let payload = |value_kind| ValuePayload {
            value_kind,
            encoded: Vec::with_capacity(4096),
        };
        super::preparation_stage("usb-hid-driver-kernel");
        let driver = artifact.prepare_pure(storage).map_err(E::Call)?;
        super::preparation_stage("usb-hid-driver-ready");
        Ok(Self {
            driver,
            pass,
            input: input.port_id,
            iterate,
            work: work.port_id,
            output: result.port_id,
            incoming: payload(input.value_kind),
            work_value: payload(work.value_kind),
            result: payload(result.value_kind),
            sequence: 0,
            remaining_submissions: 7,
            iteration_sequence: 0,
            working: false,
            ready: false,
            closed: false,
            driver_complete: false,
        })
    }
    pub fn start(&mut self) -> Result<(), PurePipeRefusal> {
        self.pass.start()?;
        self.driver.start().map_err(PurePipeRefusal::Call)
    }
    pub fn submit(&mut self, value: &ValuePayload) -> Result<(), PurePipeRefusal> {
        use PurePipeRefusal as E;
        if self.remaining_submissions == 0 {
            return Err(E::Pressure);
        }
        if self.closed
            || self.working
            || self.ready
            || value.value_kind != self.incoming.value_kind
            || value.encoded.len() > 4096
        {
            return Err(E::Binding);
        }
        let next = self.sequence.checked_add(1).ok_or(E::Binding)?;
        self.incoming.encoded.clear();
        self.incoming.encoded.extend_from_slice(&value.encoded);
        accepted(
            self.driver
                .admit_input(&self.input, self.sequence, &self.incoming)
                .map_err(E::Call)?,
        )?;
        self.sequence = next;
        self.remaining_submissions -= 1;
        self.working = true;
        Ok(())
    }
    pub fn step(&mut self) -> Result<(), PurePipeRefusal> {
        use PurePipeRefusal as E;
        if !self.pass.complete() {
            self.pass.step()?;
        }
        if let Some(value) = self.pass.result() {
            let next = self.iteration_sequence.checked_add(1).ok_or(E::Binding)?;
            accepted(
                self.driver
                    .admit_input(&self.iterate, self.iteration_sequence, value)
                    .map_err(E::Call)?,
            )?;
            self.iteration_sequence = next;
            self.pass.acknowledge();
        }
        let status = self.driver.step().map_err(E::Call)?;
        if status == conduit_composite::KernelCompositeStatus::Cancelled {
            return Err(E::Binding);
        }
        if let Some(sequence) = self
            .driver
            .output_into(&self.work, &mut self.work_value)
            .map_err(E::Call)?
        {
            if !self.working {
                return Err(E::Binding);
            }
            self.pass.submit(&self.work_value)?;
            self.driver
                .complete_output(&self.work, sequence)
                .map_err(E::Call)?;
        }
        if !self.ready
            && let Some(sequence) = self
                .driver
                .output_into(&self.output, &mut self.result)
                .map_err(E::Call)?
        {
            if !self.working {
                return Err(E::Binding);
            }
            self.driver
                .complete_output(&self.output, sequence)
                .map_err(E::Call)?;
            self.working = false;
            self.ready = true;
        }
        self.driver_complete = status == conduit_composite::KernelCompositeStatus::Complete;
        Ok(())
    }
    pub fn result(&self) -> Option<&ValuePayload> {
        self.ready.then_some(&self.result)
    }
    pub fn acknowledge(&mut self) {
        self.ready = false;
    }
    pub fn close(&mut self) -> Result<(), PurePipeRefusal> {
        if self.working || self.ready || self.closed {
            return Err(PurePipeRefusal::Binding);
        }
        self.driver
            .close_input(&self.input)
            .map_err(PurePipeRefusal::Call)?;
        self.driver
            .close_input(&self.iterate)
            .map_err(PurePipeRefusal::Call)?;
        self.pass.close()?;
        self.closed = true;
        Ok(())
    }
    pub fn complete(&self) -> bool {
        self.driver_complete && self.pass.complete()
    }
}
fn accepted(
    outcome: conduit_kernel::scheduler::RemoteIngressOutcome,
) -> Result<(), PurePipeRefusal> {
    match outcome {
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. } => Ok(()),
        _ => Err(PurePipeRefusal::Pressure),
    }
}
