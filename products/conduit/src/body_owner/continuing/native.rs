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
        owner.plan_with_execution_custody(checked, &equipment.authority_grants, &boundaries)?;
        Ok(Some(Self {
            equipment: None,
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

struct Provider<'a>(&'a mut Option<AttachedEquipment>);
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
        let AttachedEquipment {
            native,
            instruments,
            authority_grants,
        } = self
            .0
            .take()
            .expect("native worker retains actual equipment");
        let (host, native, work, result) = block_on(native.complete_native_work(host, work));
        *self.0 = Some(AttachedEquipment {
            native,
            instruments,
            authority_grants,
        });
        (
            host,
            work,
            result.map_err(|e| format!("selected native Source refused: {e:?}")),
        )
    }
}
impl NativeRun {
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
            &mut Provider(&mut self.equipment),
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
