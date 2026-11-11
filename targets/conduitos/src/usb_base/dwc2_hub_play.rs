//! Root composition of checked hub policy and independently admitted effects.
//! Descriptor, hub, HID and retry meaning remains in the retained Source.
// Runtime refusals retain full diagnostics inline without allocating.
#![allow(clippy::result_large_err)]
use super::{
    control_contract::ControlContract,
    dwc2_control::RootControlRoute,
    dwc2_control_play::{ControlPlayRefusal, Dwc2ControlPlay},
    hid_source_kernel::PreparedUsbSourceKernel,
};
use crate::{
    arch::dwc2::{Controller, RegisterIo, TransactionTranslator},
    monotonic_clock::{
        owner::MonotonicDeadlineProvider,
        play::{ClockPlay, ClockPlayRefusal},
    },
    protocol_host_calls::ProtocolCallRefusal,
    protocol_source::*,
};
use alloc::{collections::BTreeMap, vec::Vec};
use conduit_core::*;
pub(crate) enum HubPlayRefusal {
    Binding,
    Source(ProtocolSourceRefusal),
    Call(ProtocolCallRefusal),
    Control(ControlPlayRefusal),
    Clock(ClockPlayRefusal),
    Pipe(PurePipeRefusal),
    Value(StructuredInfoRefusal),
}
struct Output {
    port: PortId,
    value: ValuePayload,
}
pub(crate) struct Dwc2HubPlay<I, P> {
    run: PreparedUsbSourceKernel,
    control: Option<Dwc2ControlPlay<I>>,
    clock: ClockPlay<P>,
    configuration: PreparedPureProtocolPipe,
    descriptor: PreparedHidDescriptorPipe,
    route: RootControlRoute,
    begin: PortId,
    received: PortId,
    woke: PortId,
    topology: PortId,
    layout: PortId,
    seed: ValuePayload,
    pulse: ValuePayload,
    outputs: Vec<Output>,
    request: PreparedStructuredComposer,
    setup_first: bool,
    reply_sequence: u64,
    wake_sequence: u64,
    topology_sequence: u64,
    layout_sequence: u64,
    waiting: bool,
    finished: bool,
    complete: bool,
    result: Vec<u8>,
}
impl<I: RegisterIo, P: MonotonicDeadlineProvider> Dwc2HubPlay<I, P> {
    /// Call only during preparation. The two native Plays already retain their
    /// actual controller/clock possession; protocol metadata creates none.
    pub(crate) fn prepare(
        control: Dwc2ControlPlay<I>,
        clock: ClockPlay<P>,
        host_id: HostId,
        boot_id: BootId,
        route: RootControlRoute,
        hub_address: u8,
        first_child_address: u8,
    ) -> Result<Self, HubPlayRefusal> {
        use HubPlayRefusal as E;
        let configuration = PreparedPureProtocolPipe::prepare(
            usb_configuration_decode_package().map_err(E::Source)?,
            "usb-configuration-decode",
            host_id.clone(),
            boot_id.clone(),
        )
        .map_err(E::Pipe)?;
        let descriptor = PreparedHidDescriptorPipe::prepare(host_id.clone(), boot_id.clone())
            .map_err(E::Pipe)?;
        crate::protocol_source::preparation_stage("usb-hub-package");
        let package = usb_hub_input_package().map_err(E::Source)?;
        let entry =
            PreparedProtocolEntry::prepare_package(package, "usb-hub-input").map_err(E::Source)?;
        let seed_type = entry.input_schema(&port_id("begin")).ok_or(E::Binding)?;
        let StructuredInfoTypeShape::Record { fields, .. } = seed_type.shape() else {
            return Err(E::Binding);
        };
        let mut values = Vec::with_capacity(fields.len());
        for field in fields {
            let n = match field.name() {
                "hub-address" => hub_address,
                "first-child-address" => first_child_address,
                "maximum-packet" => route.maximum_packet,
                _ => return Err(E::Binding),
            };
            values.push(
                StructuredFieldValue::new(
                    field.name(),
                    StructuredInfoValue::leaf(field.value_type().clone(), alloc::vec![n])
                        .map_err(E::Value)?,
                )
                .map_err(E::Value)?,
            );
        }
        let seed_bytes = StructuredInfoValue::record(seed_type, values)
            .map_err(E::Value)?
            .canonical_bytes()
            .map_err(E::Value)?;
        let mut host = HostAdvertisement {
            declared_name: None,
            protocol_version: PROTOCOL_VERSION,
            host_id,
            boot_id,
            offer_generation: OfferGeneration(1),
            profile: "conduitos/hub-policy@1".into(),
            bases: alloc::vec![],
            resources: alloc::vec![],
            capabilities: alloc::vec![],
            planner_capabilities: alloc::vec![],
        };
        crate::protocol_source::preparation_stage("usb-hub-publish-backs");
        entry.publish_pure_backs(&mut host).map_err(E::Source)?;
        let hosts = [host];
        crate::protocol_source::preparation_stage("usb-hub-place");
        let placements = entry.placements(&hosts).map_err(E::Source)?;
        crate::protocol_source::preparation_stage("usb-hub-plan");
        let artifact = entry
            .plan(
                &hosts,
                &placements,
                &["conduit.base/local@1".into()],
                conduit_planner::PlanningOptions {
                    connection_bases: &BTreeMap::new(),
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: 4096,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .map_err(E::Source)?;
        crate::protocol_source::preparation_stage("usb-hub-plan-ready");
        // plan() consumes the expanded entry. Retire the remaining placement
        // scratch and offer before allocating the owned artifact's kernel.
        drop(placements);
        drop(hosts);
        crate::protocol_source::preparation_stage("usb-hub-plan-storage-retired");
        let boundary = &artifact.artifact().definition().boundary;
        let input = |name: &str| {
            boundary
                .input_fronts
                .iter()
                .find(|p| p.external_port.port_id.as_str() == name)
                .map(|p| p.external_port.clone())
                .ok_or(E::Binding)
        };
        let begin = input("begin")?;
        let received = input("received")?;
        let woke = input("woke")?;
        let topology = input("topology")?;
        let layout = input("layout")?;
        let outputs = boundary
            .output_fronts
            .iter()
            .map(|p| Output {
                port: p.external_port.port_id.clone(),
                value: ValuePayload {
                    value_kind: p.external_port.value_kind.clone(),
                    encoded: Vec::with_capacity(4096),
                },
            })
            .collect();
        let contract = ControlContract::prepare().map_err(E::Value)?;
        let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
            return Err(E::Binding);
        };
        let setup_first = fields[0].name() == "setup";
        let request =
            PreparedStructuredComposer::new(contract.request_type(), 4096).map_err(E::Value)?;
        crate::protocol_source::preparation_stage("usb-hub-kernel");
        let run =
            PreparedUsbSourceKernel::prepare_pure(artifact, Default::default()).map_err(E::Call)?;
        crate::protocol_source::preparation_stage("usb-hub-ready");
        Ok(Self {
            run,
            control: Some(control),
            clock,
            configuration,
            descriptor,
            route,
            begin: begin.port_id,
            received: received.port_id,
            woke: woke.port_id,
            topology: topology.port_id,
            layout: layout.port_id,
            seed: ValuePayload {
                value_kind: begin.value_kind,
                encoded: seed_bytes,
            },
            pulse: ValuePayload {
                value_kind: woke.value_kind,
                encoded: alloc::vec![],
            },
            outputs,
            request,
            setup_first,
            reply_sequence: 0,
            wake_sequence: 0,
            topology_sequence: 0,
            layout_sequence: 0,
            waiting: false,
            finished: false,
            complete: false,
            result: Vec::with_capacity(4096),
        })
    }
    pub(crate) fn start(&mut self) -> Result<(), HubPlayRefusal> {
        use HubPlayRefusal as E;
        self.control
            .as_mut()
            .ok_or(E::Binding)?
            .start()
            .map_err(E::Control)?;
        self.clock.start().map_err(E::Clock)?;
        self.configuration.start().map_err(E::Pipe)?;
        self.descriptor.start().map_err(E::Pipe)?;
        self.run
            .kernel_mut()
            .start()
            .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
        accepted(
            self.run
                .kernel_mut()
                .admit_input(&self.begin, 0, &self.seed),
        )?;
        self.run
            .kernel_mut()
            .close_input(&self.begin)
            .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
        Ok(())
    }
    /// One bounded quantum per retained Play. `now_ms` is a Root clock wire
    /// observation on the independently admitted clock's exact calibrated basis.
    pub(crate) fn step(&mut self, now_ms: u64) -> Result<bool, HubPlayRefusal> {
        use HubPlayRefusal as E;
        if !self.configuration.complete() {
            self.configuration.step().map_err(E::Pipe)?;
        }
        if !self.descriptor.complete() {
            self.descriptor.step().map_err(E::Pipe)?;
        }
        if let Some(value) = self.configuration.result() {
            accepted(self.run.kernel_mut().admit_input(
                &self.topology,
                self.topology_sequence,
                value,
            ))?;
            self.topology_sequence = self.topology_sequence.checked_add(1).ok_or(E::Binding)?;
            self.configuration.acknowledge();
        }
        if let Some(value) = self.descriptor.result() {
            accepted(
                self.run
                    .kernel_mut()
                    .admit_input(&self.layout, self.layout_sequence, value),
            )?;
            self.layout_sequence = self.layout_sequence.checked_add(1).ok_or(E::Binding)?;
            self.descriptor.acknowledge();
        }
        let control = self.control.as_mut().ok_or(E::Binding)?;
        control.step().map_err(E::Control)?;
        if let Some(reply) = control.result() {
            accepted(self.run.kernel_mut().admit_input(
                &self.received,
                self.reply_sequence,
                reply,
            ))?;
            self.reply_sequence = self.reply_sequence.checked_add(1).ok_or(E::Binding)?;
            control.acknowledge_result();
        }
        if self.waiting && self.clock.step().map_err(E::Clock)? {
            accepted(self.run.kernel_mut().admit_input(
                &self.woke,
                self.wake_sequence,
                &self.pulse,
            ))?;
            self.wake_sequence = self.wake_sequence.checked_add(1).ok_or(E::Binding)?;
            self.waiting = false;
        }
        let status = self
            .run
            .kernel_mut()
            .step()
            .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
        if let Some(request) = self.run.kernel_mut().next_host_request()
            && !self.run.service_pure_call(&request).map_err(E::Call)?
        {
            return Err(E::Binding);
        }
        for output in &mut self.outputs {
            let Some(sequence) = self
                .run
                .kernel_mut()
                .output_into(&output.port, &mut output.value)
                .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?
            else {
                continue;
            };
            match output.port.as_str() {
                "exchange" => {
                    let value = validate_canonical_structured_value(&output.value.encoded)
                        .map_err(E::Value)?;
                    let translator_port = octet(value, "translator-port")?;
                    let translator = if translator_port == 0 {
                        None
                    } else {
                        Some(TransactionTranslator {
                            hub_address: octet(value, "hub-address")?,
                            port: translator_port,
                            low_speed: field(value, "low-speed")?
                                .primitive_bytes("value/bool")
                                .map_err(E::Value)?
                                == [1],
                        })
                    };
                    let route = RootControlRoute {
                        address: octet(value, "address")?,
                        maximum_packet: octet(value, "maximum-packet")?,
                        translator,
                        ..self.route
                    };
                    let request = field(value, "request")?;
                    let setup = field(request, "setup")?;
                    let data = field(request, "output")?;
                    let fields = if self.setup_first {
                        [setup, data]
                    } else {
                        [data, setup]
                    };
                    let encoded = self.request.record(&fields).map_err(E::Value)?;
                    control.submit(route, encoded).map_err(E::Control)?;
                }
                "configuration-decode" => {
                    self.configuration.submit(&output.value).map_err(E::Pipe)?
                }
                "descriptor-decode" => self.descriptor.submit(&output.value).map_err(E::Pipe)?,
                "wait" => {
                    if self.waiting {
                        return Err(E::Binding);
                    }
                    let delay = u64::from_le_bytes(
                        output
                            .value
                            .encoded
                            .as_slice()
                            .try_into()
                            .map_err(|_| E::Binding)?,
                    );
                    self.clock
                        .submit(now_ms.checked_add(delay).ok_or(E::Binding)?)
                        .map_err(E::Clock)?;
                    self.waiting = true;
                }
                "result" => {
                    if self.finished {
                        return Err(E::Binding);
                    }
                    self.result.extend_from_slice(&output.value.encoded);
                    self.finished = true;
                    self.run
                        .kernel_mut()
                        .close_input(&self.received)
                        .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
                    self.run
                        .kernel_mut()
                        .close_input(&self.woke)
                        .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
                    self.run
                        .kernel_mut()
                        .close_input(&self.topology)
                        .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
                    self.run
                        .kernel_mut()
                        .close_input(&self.layout)
                        .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
                    self.configuration.close().map_err(E::Pipe)?;
                    self.descriptor.close().map_err(E::Pipe)?;
                }
                _ => return Err(E::Binding),
            }
            self.run
                .kernel_mut()
                .complete_output(&output.port, sequence)
                .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
        }
        self.complete = self.finished
            && status == conduit_composite::KernelCompositeStatus::Complete
            && self.configuration.complete()
            && self.descriptor.complete();
        Ok(self.complete)
    }
    pub(crate) fn waiting_for_clock(&self) -> bool {
        self.waiting && self.clock.pending()
    }
    pub(crate) fn result(&self) -> &[u8] {
        &self.result
    }
    /// Keep the entire owner on refusal; unsettled native custody is never discarded.
    #[allow(clippy::result_large_err)]
    pub(crate) fn retire_controller(mut self) -> Result<Controller<I>, (Self, HubPlayRefusal)> {
        if !self.complete || self.waiting {
            return Err((self, HubPlayRefusal::Binding));
        }
        let control = self.control.take().expect("original control owner");
        match control.retire() {
            Ok(controller) => Ok(controller),
            Err((control, e)) => {
                self.control = Some(control);
                Err((self, HubPlayRefusal::Control(e)))
            }
        }
    }
}
fn accepted(
    result: Result<
        conduit_kernel::scheduler::RemoteIngressOutcome,
        conduit_composite::KernelCompositeError,
    >,
) -> Result<(), HubPlayRefusal> {
    match result.map_err(|e| HubPlayRefusal::Call(ProtocolCallRefusal::Kernel(e)))? {
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. } => Ok(()),
        _ => Err(HubPlayRefusal::Binding),
    }
}
fn field<'a>(
    value: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> Result<ValidatedCanonicalStructuredValue<'a>, HubPlayRefusal> {
    value
        .record_field(name)
        .map_err(HubPlayRefusal::Value)?
        .ok_or(HubPlayRefusal::Binding)
}
fn octet(value: ValidatedCanonicalStructuredValue<'_>, name: &str) -> Result<u8, HubPlayRefusal> {
    let bytes = field(value, name)?
        .primitive_bytes("value/u8")
        .map_err(HubPlayRefusal::Value)?;
    bytes.first().copied().ok_or(HubPlayRefusal::Binding)
}

impl core::fmt::Debug for HubPlayRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Binding => f.write_str("Binding"),
            Self::Source(error) => f.debug_tuple("Source").field(error).finish(),
            Self::Call(error) => f.debug_tuple("Call").field(error).finish(),
            Self::Control(error) => f.debug_tuple("Control").field(error).finish(),
            Self::Clock(error) => f.debug_tuple("Clock").field(error).finish(),
            Self::Pipe(error) => f.debug_tuple("Pipe").field(error).finish(),
            Self::Value(error) => f.debug_tuple("Value").field(error).finish(),
        }
    }
}
