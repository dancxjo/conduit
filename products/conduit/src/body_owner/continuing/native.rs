//! Installed equipment for one checked environmental Source encounter.
//! The ordinary Body kernel retains execution, Fore and cancellation custody.
use super::*;
use crate::durable_host::selected_environmental_sources::AttachedEquipment;
use conduit_environment_measurement::source_observation::{ObservationContract, MAXIMUM_BYTES};
use conduit_std_host::{body_execution::BodyForeOutputAdapter, ExternalForeDelivery};
use std::collections::BTreeMap;

pub(super) struct NativeRun {
    pub equipment: Option<AttachedEquipment>,
    pub inputs: Vec<conduit_std_host::ExternalForeInput>,
    pub capture: Capture,
    selection: SourceSelection,
    completed: Option<conduit_core::ActivePlayIdentity>,
}
#[derive(serde::Serialize)]
pub(super) struct SourceSelection {
    attachment_id: String,
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
    capability_id: conduit_core::CapabilityId,
    provider_instance_id: conduit_core::BaseInstanceId,
    provider_generation: u64,
    plan_id: conduit_core::PlanId,
    fragment_id: conduit_core::FragmentId,
    placement_id: conduit_core::PlacementId,
    configuration_id: String,
    firmware_sha256: [u8; 32],
}
pub(super) struct AcceptedSourceFore {
    pub value: ExternalForeDelivery,
    pub selection: SourceSelection,
    pub native_play: conduit_core::ActivePlayIdentity,
}
pub(super) struct Capture {
    port: conduit_core::PortId,
    kind: conduit_core::KindId,
    contract: ObservationContract,
    pub value: Option<ExternalForeDelivery>,
}
impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, value: &ExternalForeDelivery) -> Result<(), String> {
        if self.value.is_some()
            || value.front_port_id != self.port
            || value.track != conduit_core::ConnectionTrack::Payload
            || value.value_kind != self.kind
            || self.contract.decode(&value.bytes).is_err()
        {
            return Err("selected environmental Source returned an unexpected Fore".into());
        }
        // Prepared delivery storage is shared, not copied or acknowledged as a
        // durable recording. The containing terminal receipt supplies Play scope.
        self.value = Some(value.clone());
        Ok(())
    }
}
impl NativeRun {
    pub fn prepare(
        owner: &mut Owner,
        checked: &conduit_plot::ExpandedAuthoringPlot,
        equipment: &AttachedEquipment,
    ) -> Result<Option<Self>, String> {
        // Attaching equipment does not change the meaning or execution path of
        // unrelated Plots on this Host. Select this provider by checked Kind.
        if !checked.expanded.gears.iter().any(|gear| {
            gear.kind_id.as_str()
                == conduit_environment_measurement::source_observation::SOURCE_KIND
        }) {
            return Ok(None);
        }
        equipment.validate_owner(&owner.host)?;
        let contract = ObservationContract::prepare().map_err(super::super::debug)?;
        let kind = contract.kind(false).map_err(super::super::debug)?;
        let [input] = checked.front.inputs() else {
            return Err("selected Source encounter requires one explicit Empty trigger".into());
        };
        let [output] = checked.front.outputs() else {
            return Err("selected Source encounter requires one typed observation Fore".into());
        };
        if input.value_kind.as_str() != conduit_core::EMPTY_INFO_ID
            || output.value_kind != kind.outputs[0].value_kind
            || checked.input_bindings.len() != 1
            || checked.output_bindings.len() != 1
            || checked.input_bindings[0].track != conduit_core::ConnectionTrack::Payload
            || checked.output_bindings[0].track != conduit_core::ConnectionTrack::Payload
            || checked.input_bindings[0].gear_id != checked.output_bindings[0].gear_id
            || checked.input_bindings[0].gear_port_id != kind.inputs[0].port_id
            || checked.output_bindings[0].gear_port_id != kind.outputs[0].port_id
        {
            return Err("selected Source encounter has unsupported external Fore contracts".into());
        }
        let boundaries = BTreeMap::from([
            (
                conduit_planner::ForeBoundaryKey {
                    direction: conduit_core::PortDirection::Input,
                    front_port_id: input.port_id.clone(),
                    track: conduit_core::ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 1,
                },
            ),
            (
                conduit_planner::ForeBoundaryKey {
                    direction: conduit_core::PortDirection::Output,
                    front_port_id: output.port_id.clone(),
                    track: conduit_core::ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: MAXIMUM_BYTES as u32,
                },
            ),
        ]);
        let resident = owner.resident.as_ref().ok_or("Body has no resident Plot")?;
        let partition = owner.plan_partition_with_authority_grants(
            checked,
            resident,
            &equipment.authority_grants,
            &boundaries,
        )?;
        let mut matches = partition.plan.fragments.iter().flat_map(|fragment| {
            fragment.placements.iter().filter_map(move |placement| {
                (placement.gear_id == checked.output_bindings[0].gear_id)
                    .then_some((fragment, placement))
            })
        });
        let (fragment, placement) = matches.next().ok_or("Source output placement absent")?;
        if matches.next().is_some() || placement.kind_id != kind.kind_id {
            return Err("Source output has ambiguous native provenance".into());
        }
        let base = placement
            .base
            .as_ref()
            .ok_or("Source output has no selected Base")?;
        let mut instruments = equipment.instruments.iter().filter(|instrument| {
            instrument.session.host() == &placement.host_id
                && instrument.session.boot() == &placement.boot_id
                && instrument.session.capability() == &placement.capability_id
                && instrument.session.session() == &base.provider_instance_id
        });
        let instrument = instruments
            .next()
            .ok_or("Source output has no retained equipment session")?;
        if instruments.next().is_some() {
            return Err("Source output has multiple equipment associations".into());
        }
        let selection = SourceSelection {
            attachment_id: instrument.attachment_id.clone(),
            host_id: placement.host_id.clone(),
            boot_id: placement.boot_id.clone(),
            capability_id: placement.capability_id.clone(),
            provider_instance_id: base.provider_instance_id.clone(),
            provider_generation: base.provider_generation,
            plan_id: fragment.plan_id.clone(),
            fragment_id: fragment.fragment_id.clone(),
            placement_id: placement.placement_id.clone(),
            configuration_id: instrument.source_configuration.configuration_id().into(),
            firmware_sha256: instrument.firmware,
        };
        // Do not publish a proposal until its output is associated with the
        // exact independently retained equipment, rather than vector position.
        let ad = owner.host.advertisement();
        owner
            .session
            .propose(vec![partition], &ad.host_id, &ad.boot_id)
            .map_err(super::super::debug)?;
        Ok(Some(Self {
            equipment: None,
            selection,
            completed: None,
            inputs: vec![conduit_std_host::ExternalForeInput {
                front_port_id: input.port_id.clone(),
                track: conduit_core::ConnectionTrack::Payload,
                bytes: vec![],
            }],
            capture: Capture {
                port: output.port_id.clone(),
                kind: output.value_kind.clone(),
                contract,
                value: None,
            },
        }))
    }
}

struct Provider<'a> {
    equipment: &'a mut Option<AttachedEquipment>,
    selection: &'a SourceSelection,
    completed: &'a mut Option<conduit_core::ActivePlayIdentity>,
}
impl SourceSelection {
    fn matches(
        &self,
        selected: &conduit_plan_lowering::selected_operation::SelectedOperationPlan<'_>,
    ) -> bool {
        selected.fragment.plan_id == self.plan_id
            && selected.fragment.fragment_id == self.fragment_id
            && selected.fragment.host_id == self.host_id
            && selected.fragment.boot_id == self.boot_id
            && selected.placement_id == &self.placement_id
            && selected.fragment.placements.iter().any(|placement| {
                placement.placement_id == self.placement_id
                    && placement.capability_id == self.capability_id
                    && placement.base.as_ref().is_some_and(|base| {
                        base.provider_instance_id == self.provider_instance_id
                            && base.provider_generation == self.provider_generation
                    })
            })
    }
}
impl conduit_std_host::body_execution::native::BodyNativeProvider for Provider<'_> {
    fn execute(
        &mut self,
        host: StdHost,
        work: conduit_std_host::PreparedNativeWork,
    ) -> (
        StdHost,
        conduit_std_host::PreparedNativeWork,
        Result<Vec<u8>, String>,
    ) {
        let selected_source = self.selection.matches(&work.selected());
        let AttachedEquipment {
            native,
            instruments,
            authority_grants,
        } = self
            .equipment
            .take()
            .expect("native worker retains actual equipment");
        let (host, native, work, result) = block_on(native.complete_native_work(host, work));
        *self.equipment = Some(AttachedEquipment {
            native,
            instruments,
            authority_grants,
        });
        let result = result.map_err(|e| format!("selected native Source refused: {e:?}"));
        if selected_source && result.is_ok() {
            if !self.selection.matches(&work.selected()) {
                return (
                    host,
                    work,
                    Err("native Source returned a different selection".into()),
                );
            }
            *self.completed = Some(work.selected().active.clone());
        }
        (host, work, result)
    }
}
impl NativeRun {
    pub fn accepted(self) -> Result<Option<AcceptedSourceFore>, String> {
        self.capture
            .value
            .map(|value| {
                Ok(AcceptedSourceFore {
                    value,
                    selection: self.selection,
                    native_play: self
                        .completed
                        .ok_or("Source Fore has no exact native completion")?,
                })
            })
            .transpose()
    }
    pub fn execute<W: std::io::Write, T: conduit_std_host::TimerAdapter, F>(
        &mut self,
        host: StdHost,
        request: BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> (StdHost, Result<BodyRunReport, String>)
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        host.run_body_plan_owned_native_to(
            request,
            &self.inputs,
            &mut self.capture,
            output,
            timer,
            &mut Provider {
                equipment: &mut self.equipment,
                selection: &self.selection,
                completed: &mut self.completed,
            },
            started,
        )
    }
}
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct Wake(std::thread::Thread);
    impl std::task::Wake for Wake {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark()
        }
        fn wake_by_ref(self: &std::sync::Arc<Self>) {
            self.0.unpark()
        }
    }
    let waker = std::task::Waker::from(std::sync::Arc::new(Wake(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => return value,
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_environment_measurement::source_observation::{
        PreparedObservationCodec, SourceFailure, SourceObservation, SourceOutcome,
    };
    #[test]
    fn accepted_fore_is_exact_typed_storage_and_duplicate_does_not_replace_it() {
        let contract = ObservationContract::prepare().unwrap();
        let kind = contract.kind(false).unwrap().outputs[0].value_kind.clone();
        let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
        let bytes = codec
            .encode(SourceObservation {
                observed_at_host_boot_ms: 42,
                outcome: SourceOutcome::Unavailable(SourceFailure::CaptureTimeout),
            })
            .unwrap()
            .to_vec();
        let mut capture = Capture {
            port: "reading".into(),
            kind: kind.clone(),
            contract,
            value: None,
        };
        for (port, value_kind, data) in [
            ("other", kind.clone(), bytes.clone()),
            ("reading", "Bool".into(), bytes.clone()),
            ("reading", kind.clone(), vec![0; MAXIMUM_BYTES + 1]),
        ] {
            let wrong = ExternalForeDelivery::new(
                port.into(),
                conduit_core::ConnectionTrack::Payload,
                value_kind,
                0,
                data,
            );
            assert!(capture.deliver(&wrong).is_err());
            assert!(capture.value.is_none());
        }
        let delivery = ExternalForeDelivery::new(
            "reading".into(),
            conduit_core::ConnectionTrack::Payload,
            kind,
            0,
            bytes,
        );
        capture.deliver(&delivery).unwrap();
        assert_eq!(capture.value.as_ref(), Some(&delivery));
        assert!(capture.deliver(&delivery).is_err());
        assert_eq!(capture.value.as_ref(), Some(&delivery));
    }
}
