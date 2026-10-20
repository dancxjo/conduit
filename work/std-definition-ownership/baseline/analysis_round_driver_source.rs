//! Actual local Source Plays, with moved private requests retained across failure.
use super::*;
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
use std::{collections::BTreeMap, ops::Range, sync::Arc};

pub(super) struct Operation {
    pub name: &'static str,
    pub definition: Arc<KernelCompositeDefinition>,
    pub kernel: KernelCompositeHost,
    pub plays: BTreeMap<HostId, ActivePlayId>,
    input_port: PortId,
    output_port: PortId,
    input: ValuePayload,
    output: ValuePayload,
    sequence: u64,
    pub error: Option<KernelCompositeError>,
    pub unexpected: Option<conduit_composite::KernelCompositeHostRequest>,
}
impl Operation {
    pub fn prepare(
        name: &'static str,
        definition: KernelCompositeDefinition,
        signs: KernelCompositeSignStorage,
    ) -> Result<Self, DriverRefusal> {
        let input_front = &definition.boundary.input_fronts[0];
        let output_front = &definition.boundary.output_fronts[0];
        let capacity = |direction| {
            definition.internal_plan.fragments[0]
                .fore_ports
                .iter()
                .find(|port| port.direction == direction)
                .map(|port| port.byte_capacity as usize)
                .ok_or_else(|| DriverRefusal::PreparationDetail {
                    stage: "kernel-fore-capacity",
                    detail: format!("{name}: {direction:?}"),
                })
        };
        let input = ValuePayload {
            value_kind: input_front.external_port.value_kind.clone(),
            encoded: Vec::with_capacity(capacity(PortDirection::Input)?),
        };
        let output = ValuePayload {
            value_kind: output_front.external_port.value_kind.clone(),
            encoded: Vec::with_capacity(capacity(PortDirection::Output)?),
        };
        let input_port = input_front.external_port.port_id.clone();
        let output_port = output_front.external_port.port_id.clone();
        let definition = Arc::new(definition);
        let mut registry = KernelOperationRegistry::new();
        registry
            .install(crate::flow_activation::LocalExpressionFactory)
            .map_err(|error| DriverRefusal::PreparationDetail {
                stage: "kernel-preparation",
                detail: format!("{name}: {error:?}"),
            })?;
        let mut kernel =
            KernelCompositeHost::prepare_with_sign_storage((*definition).clone(), &registry, signs)
                .map_err(|error| DriverRefusal::PreparationDetail {
                    stage: "kernel-preparation",
                    detail: format!("{name}: {error:?}"),
                })?;
        kernel
            .start()
            .map_err(|error| DriverRefusal::PreparationDetail {
                stage: "kernel-preparation",
                detail: format!("{name}: {error:?}"),
            })?;
        let plays = kernel.active_plays().clone();
        Ok(Self {
            name,
            definition,
            plays,
            kernel,
            input_port,
            output_port,
            input,
            output,
            sequence: 0,
            error: None,
            unexpected: None,
        })
    }
    pub fn submit(&mut self, input: &[u8]) -> Result<(), DriverRefusal> {
        if input.len() > self.input.encoded.capacity() || self.sequence == u64::MAX {
            return Err(DriverRefusal::Pressure);
        }
        self.input.encoded.clear();
        self.input.encoded.extend_from_slice(input);
        match self
            .kernel
            .admit_input(&self.input_port, self.sequence, &self.input)
        {
            Ok(RemoteIngressOutcome::Accepted { sequence }) if sequence == self.sequence => Ok(()),
            Ok(RemoteIngressOutcome::Full { .. }) => Err(DriverRefusal::Pressure),
            Err(error) => {
                self.error = Some(error);
                Err(DriverRefusal::Kernel)
            }
            _ => Err(DriverRefusal::Sequence),
        }
    }
    /// One actual ordinary kernel step, followed by non-advancing output observation.
    pub fn poll(&mut self) -> Result<Option<&[u8]>, DriverRefusal> {
        match self.kernel.step() {
            Ok(KernelCompositeStatus::Active) => (),
            Err(error) => {
                self.error = Some(error);
                return Err(DriverRefusal::Kernel);
            }
            _ => return Err(DriverRefusal::Kernel),
        }
        if let Some(request) = self.kernel.next_host_request() {
            self.unexpected = Some(request);
            return Err(DriverRefusal::UnexpectedHostCall);
        }
        match self.kernel.output_into(&self.output_port, &mut self.output) {
            Ok(Some(sequence)) if sequence == self.sequence => Ok(Some(&self.output.encoded)),
            Ok(None) => Ok(None),
            Err(error) => {
                self.error = Some(error);
                Err(DriverRefusal::Kernel)
            }
            _ => Err(DriverRefusal::Sequence),
        }
    }
    pub fn retained_output(&self) -> &[u8] {
        &self.output.encoded
    }
    pub fn acknowledge(&mut self) -> Result<u64, DriverRefusal> {
        if let Err(error) = self
            .kernel
            .complete_output(&self.output_port, self.sequence)
        {
            self.error = Some(error);
            return Err(DriverRefusal::Kernel);
        }
        let sequence = self.sequence;
        self.sequence += 1;
        Ok(sequence)
    }
    pub fn cancel(&mut self) -> Result<(), DriverRefusal> {
        self.kernel.cancel().map_err(|error| {
            self.error = Some(error);
            DriverRefusal::Kernel
        })
    }
}
pub(super) struct Record {
    pub operation: usize,
    pub ordinal: usize,
    pub input: Range<usize>,
    pub output: Range<usize>,
    pub ack: Option<u64>,
    pub submitted: bool,
}
/// View of actual Source execution evidence. ACK is not Native-law admission.
pub struct SourceExecution<'a> {
    pub operation: &'static str,
    pub ordinal: usize,
    pub input: &'a [u8],
    pub output: &'a [u8],
    pub acknowledged_sequence: Option<u64>,
    pub submitted_to_cursor: bool,
    pub definition: &'a KernelCompositeDefinition,
    pub active_plays: &'a BTreeMap<HostId, ActivePlayId>,
}
pub(super) struct Pending {
    pub operation: usize,
    pub record: usize,
    pub lease: Option<CursorSourceLease>,
    pub admitted: bool,
    pub acked: bool,
}
