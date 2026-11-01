//! Installed equipment for one checked environmental Source encounter.
//! The ordinary Body kernel retains execution, Fore and cancellation custody.
use super::*;
use crate::durable_host::selected_environmental_sources::AttachedEquipment;
use conduit_environment_measurement::source_observation::{ObservationContract, MAXIMUM_BYTES};
use conduit_std_host::{body_execution::BodyForeOutputAdapter, ExternalForeDelivery};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
#[path = "native/live.rs"]
mod live;
pub(in super::super) use live::{LiveCapture, LiveFore};

pub(super) struct NativeRun {
    pub equipment: Option<AttachedEquipment>,
    pub inputs: Vec<conduit_std_host::ExternalForeInput>,
    pub capture: Capture,
    selection: Arc<SourceSelection>,
    completed: Arc<Mutex<Option<Arc<CompletedScope>>>>,
    pub live: Option<LiveCapture>,
}
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
pub(in super::super) struct SourceSelection {
    pub(in super::super) attachment_id: String,
    pub(in super::super) host_id: conduit_core::HostId,
    pub(in super::super) boot_id: conduit_core::BootId,
    pub(in super::super) capability_id: conduit_core::CapabilityId,
    pub(in super::super) provider_instance_id: conduit_core::BaseInstanceId,
    pub(in super::super) provider_generation: u64,
    pub(in super::super) plan_id: conduit_core::PlanId,
    pub(in super::super) fragment_id: conduit_core::FragmentId,
    pub(in super::super) placement_id: conduit_core::PlacementId,
    pub(in super::super) configuration_id: String,
    pub(in super::super) firmware_sha256: [u8; 32],
}
pub(in super::super) struct CompletedScope {
    pub selection: Arc<SourceSelection>,
    pub native_play: conduit_core::ActivePlayIdentity,
}
pub(super) struct AcceptedSourceFore {
    pub value: ExternalForeDelivery,
    pub selection: Arc<SourceSelection>,
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
        let input = match checked.front.inputs() {
            [] if checked.input_bindings.is_empty() => None,
            [input] => Some(input),
            _ => {
                return Err(
                    "selected Source requires an explicit Empty trigger or authored cadence".into(),
                )
            }
        };
        let [output] = checked.front.outputs() else {
            return Err("selected Source encounter requires one typed observation Fore".into());
        };
        if input.is_some_and(|input| input.value_kind.as_str() != conduit_core::EMPTY_INFO_ID)
            || output.value_kind != kind.outputs[0].value_kind
            || checked.input_bindings.len() != usize::from(input.is_some())
            || checked.output_bindings.len() != 1
            || checked
                .input_bindings
                .first()
                .is_some_and(|binding| binding.track != conduit_core::ConnectionTrack::Payload)
            || checked.output_bindings[0].track != conduit_core::ConnectionTrack::Payload
            || checked
                .input_bindings
                .first()
                .is_some_and(|binding| binding.gear_id != checked.output_bindings[0].gear_id)
            || checked
                .input_bindings
                .first()
                .is_some_and(|binding| binding.gear_port_id != kind.inputs[0].port_id)
            || checked.output_bindings[0].gear_port_id != kind.outputs[0].port_id
        {
            return Err("selected Source encounter has unsupported external Fore contracts".into());
        }
        let mut boundaries = BTreeMap::from([(
            conduit_planner::ForeBoundaryKey {
                direction: conduit_core::PortDirection::Output,
                front_port_id: output.port_id.clone(),
                track: conduit_core::ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: MAXIMUM_BYTES as u32,
            },
        )]);
        if let Some(input) = input {
            boundaries.insert(
                conduit_planner::ForeBoundaryKey {
                    direction: conduit_core::PortDirection::Input,
                    front_port_id: input.port_id.clone(),
                    track: conduit_core::ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 1,
                },
            );
        }
        let resident = owner.resident.as_ref().ok_or("Body has no resident Plot")?;
        let partition = owner.plan_partition_with_limits(
            checked,
            resident,
            &equipment.authority_grants,
            &boundaries,
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: conduit_time::TICK_ENCODED_LEN,
            },
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
        let selection = Arc::new(SourceSelection {
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
        });
        #[cfg(feature = "installed-body-storage")]
        if owner.host.selected_body_storage().is_some() {
            let body_id = &owner.session.evidence().body_id;
            match owner.source_recording_stage.as_ref() {
                Some(stage) if stage.len() > 0 => {
                    stage.check_selection(body_id, &selection).map_err(debug)?
                }
                _ => {
                    owner.source_recording_stage = Some(
                        super::source_recording_stage::SourceRecordingStage::prepare(
                            body_id.clone(),
                            selection.clone(),
                        )
                        .map_err(debug)?,
                    );
                }
            }
        }
        // Do not publish a proposal until its output is associated with the
        // exact independently retained equipment, rather than vector position.
        if owner.environmental_face.is_none() {
            owner.environmental_face =
                Some(super::super::environmental_face::EnvironmentalFace::prepare()?);
        }
        owner
            .environmental_face
            .as_mut()
            .expect("prepared Source Face")
            .select(
                selection.clone(),
                instrument
                    .source_configuration
                    .configuration()
                    .metadata
                    .as_ref()
                    .map(|metadata| metadata.placement.clone()),
            )?;
        let ad = owner.host.advertisement();
        owner
            .session
            .propose(vec![partition], &ad.host_id, &ad.boot_id)
            .map_err(super::super::debug)?;
        Ok(Some(Self {
            equipment: None,
            selection,
            completed: Arc::new(Mutex::new(None)),
            live: if input.is_none() {
                Some(LiveCapture::prepare(
                    output.port_id.clone(),
                    output.value_kind.clone(),
                )?)
            } else {
                None
            },
            inputs: input
                .into_iter()
                .map(|input| conduit_std_host::ExternalForeInput {
                    front_port_id: input.port_id.clone(),
                    track: conduit_core::ConnectionTrack::Payload,
                    bytes: vec![],
                })
                .collect(),
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
    selection: &'a Arc<SourceSelection>,
    completed: &'a Arc<Mutex<Option<Arc<CompletedScope>>>>,
    failure_category: conduit_std_host::body_execution::native::NativeFailureCategory,
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
    fn failure_category(&self) -> conduit_std_host::body_execution::native::NativeFailureCategory {
        self.failure_category
    }

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
        self.failure_category =
            result.as_ref().err().map(category).unwrap_or(
                conduit_std_host::body_execution::native::NativeFailureCategory::Unknown,
            );
        let result = result.map_err(|_| "selected native Source refused".into());
        if selected_source && result.is_ok() {
            if !self.selection.matches(&work.selected()) {
                self.failure_category =
                    conduit_std_host::body_execution::native::NativeFailureCategory::Authorization;
                return (
                    host,
                    work,
                    Err("native Source returned a different selection".into()),
                );
            }
            let mut completed = self.completed.lock().expect("native scope lock");
            if completed
                .as_ref()
                .is_some_and(|scope| &scope.native_play != work.selected().active)
            {
                self.failure_category =
                    conduit_std_host::body_execution::native::NativeFailureCategory::Authorization;
                return (
                    host,
                    work,
                    Err("Source Play changed inside retained execution".into()),
                );
            }
            if completed.is_none() {
                *completed = Some(Arc::new(CompletedScope {
                    selection: self.selection.clone(),
                    native_play: work.selected().active.clone(),
                }));
            }
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
                        .lock()
                        .map_err(|_| "native scope poisoned")?
                        .as_ref()
                        .ok_or("Source Fore has no exact native completion")?
                        .native_play
                        .clone(),
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
        let mut provider = Provider {
            equipment: &mut self.equipment,
            selection: &self.selection,
            completed: &self.completed,
            failure_category:
                conduit_std_host::body_execution::native::NativeFailureCategory::Unknown,
        };
        if let Some(live) = &mut self.live {
            live.scope = self.completed.clone();
            return host.run_body_plan_owned_native_borrowed_to(
                request,
                &self.inputs,
                live,
                output,
                timer,
                &mut provider,
                started,
            );
        }
        host.run_body_plan_owned_native_to(
            request,
            &self.inputs,
            &mut self.capture,
            output,
            timer,
            &mut provider,
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

fn category(
    error: &conduit_environment_host::source_operator::Refusal,
) -> conduit_std_host::body_execution::native::NativeFailureCategory {
    use conduit_environment_host::source_operator::Refusal as R;
    use conduit_environment_host::source_operator::SourceRefusal as S;
    use conduit_std_host::body_execution::native::NativeFailureCategory as C;
    match error {
        R::Authority | R::Advertisement | R::WrongBoot | R::Plan | R::Planning(_) => {
            C::Authorization
        }
        R::Possession => C::Retired,
        R::Capacity => C::Capacity,
        R::Acquisition => C::Acquisition,
        R::Source(source) => match source {
            S::Binding(_) | S::Capability(_) | S::Possession | S::Request => C::Authorization,
            S::Capacity => C::Capacity,
            S::TransportTimeout => C::TransportTimeout,
            S::TransportIo => C::TransportIo,
            S::ConnectionRefused => C::ConnectionRefused,
            S::ProviderSpawn => C::ProviderSpawn,
            S::ProviderProtocol => C::ProviderProtocol,
            S::ProviderRefused => C::ProviderRefused,
            S::MalformedResponse => C::MalformedResponse,
            S::Identity => C::SourceIdentity,
            S::MalformedObservation | S::Encoding(_) => C::MalformedObservation,
            S::Continuity => C::SourceContinuity,
            S::Provider => C::Acquisition,
        },
    }
}
