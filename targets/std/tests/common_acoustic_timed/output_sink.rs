//! Test-only finite PCM commit boundary. DSP remains the authored Source patch.
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostedValueStore,
    PortId, RequestId,
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub const IMPLEMENTATION: &str = "fixture/pcm-commit@1";
pub fn install(
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> CapabilityOffer {
    let port = |name: &str, direction| PortDescriptor {
        port_id: name.into(),
        value_kind: kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let contract =
        CheckedValueContract::new(kind_id(conduit_audio::AUDIO_PCM_INFO_ID), 2077, vec![]).unwrap();
    let kind = Kind {
        kind_id: "fixture/commit-pcm".into(),
        kind_contract_revision: "fixture/commit-pcm@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![port("frames", PortDirection::Input)],
        outputs: vec![port("committed", PortDirection::Output)],
        configuration: vec![],
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![
            FrontValueContract {
                location: FrontValueLocation::Input("frames".into()),
                contract: contract.clone(),
            },
            FrontValueContract {
                location: FrontValueLocation::Output("committed".into()),
                contract,
            },
        ])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            // The activation planner shares the admitted wide child queue default.
            // PCM payload and Host Call contracts remain exactly 2077 bytes.
            max_queue_bytes: 131072,
        },
    };
    startup
        .insert(conduit_plot::KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
        .insert_fore(kind.kind_id.as_str(), kind.checked_front())
        .unwrap();
    profiles.insert_kind(kind.clone()).unwrap();
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: IMPLEMENTATION.into(),
            execution_profile_id: IMPLEMENTATION.into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: IMPLEMENTATION.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: "fixture/pcm-commit-call@1".into(),
                target_kind: None,
                maximum_in_flight: 1,
                maximum_input_bytes: 2077,
                maximum_output_bytes: 2077,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
pub struct Factory;
impl KernelOperationFactory for Factory {
    fn implementation_id(&self) -> &ImplementationId {
        static ID: std::sync::OnceLock<ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| IMPLEMENTATION.into())
    }
    fn budget(&self, _: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 3,
            value_bytes: 6231,
            maximum_value_bytes: 2077,
            host_requests: 1,
            sign_items: 16,
        })
    }
    fn prepare(
        &self,
        _: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        Ok(Box::new(Sink {
            pending: false,
            committed: 0,
        }))
    }
}
struct Sink {
    pending: bool,
    committed: u16,
}
impl<const P: usize> StepBack<P> for Sink {
    fn step(&mut self, io: &mut StepIo<P>, _: &StepInputBytes<'_, P>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(u32::from(self.committed)) {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 1800,
                });
            }
            if outcome.disposition == HostCallDisposition::Failed {
                return StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                    code: FailureCode::HostCallFailed,
                    detail: 75,
                }));
            }
            if outcome.disposition != HostCallDisposition::Completed {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 1801,
                });
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let value = outcome
                .output
                .expect("fixture provider returns committed PCM")
                .value;
            io.consume_host_completion().unwrap();
            io.send(PortId(0), value).unwrap();
            self.pending = false;
            self.committed += 1;
            return StepOutcome::Progress;
        }
        if self.pending {
            return StepOutcome::Await;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.committed >= 4 {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 1802,
                });
            }
            io.request_host_call(
                RequestId(u32::from(self.committed)),
                HostCallId(0),
                BoundedValueRef::new(value, 2077).unwrap(),
            )
            .unwrap();
            io.consume(PortId(0)).unwrap();
            self.pending = true;
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).unwrap();
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
}
