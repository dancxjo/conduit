//! Internal supplementary terminal effect, separate from the initial Show ACK.
use crate::flow_activation::parsec_instruction_source_offer as source;
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId as KernelPortId,
    RequestId,
};
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};

pub(super) const KIND: &str = "parsec/input-instruction-effect";
pub(super) const CONTRACT: &str = "conduit.host/parsec-input-instruction-effect@1";
pub(super) const IMPLEMENTATION: &str = "std/parsec-input-instruction-effect@1";

/// One admitted supplementary operation slot on the attached provider; it is
/// not another terminal device or an increase in the initial renderer pool.
pub(super) const RESOURCE: &str = "conduit.resource/terminal-instruction-operation@1";
pub(super) fn resource() -> ResourceOffer {
    resource_offer("terminal/instruction-operation", RESOURCE, 1)
}
pub(super) fn semantic_kind() -> Result<Kind, String> {
    Ok(Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(KIND),
        kind_contract_revision: "conduit.parsec/input-instruction-effect@1".into(),
        inputs: vec![PortDescriptor {
            port_id: port_id("instruction"),
            value_kind: source::instruction_kind()?,
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: vec![],
        configuration: vec![],
        semantic_laws: vec![KindSemanticLaw::Terminal(
            KindTerminalBehavior::EmitsOnceWhenScopeIsEligible,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: super::MAX_TERMINAL_VALUE_BYTES,
        },
    })
}
pub(super) fn offer() -> Result<CapabilityOffer, String> {
    Ok(BackOfferBuilder::new(
        semantic_kind()?,
        Back {
            capability_id: IMPLEMENTATION.into(),
            execution_profile_id: "std/terminal-mask@1".into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: "std/terminal-mask@1".into(),
            host_calls: vec![HostCallRequirement {
                contract_id: CONTRACT.into(),
                target_kind: Some(source::instruction_kind()?),
                maximum_in_flight: 1,
                maximum_input_bytes: source::MAXIMUM_INSTRUCTION_BYTES,
                maximum_output_bytes: 0,
            }],
            resource_requirements: vec![resource_requirement(RESOURCE, 1)],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub(super) fn install_catalogs(
    startup: &mut StartupCatalog,
    catalog: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: KIND.into(),
        startup_parameters: vec![],
    })?;
    catalog
        .insert_kind(semantic_kind()?)
        .map_err(|e| format!("{e:?}"))
}

pub(super) struct InstructionEffectBack {
    pending: bool,
    complete: bool,
    acknowledged: bool,
}
impl InstructionEffectBack {
    pub(super) fn acknowledged(&self) -> bool {
        self.acknowledged
    }
    pub(super) fn new() -> Self {
        Self {
            pending: false,
            complete: false,
            acknowledged: false,
        }
    }
}
impl<const PORTS: usize> StepBack<PORTS> for InstructionEffectBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let failure = || {
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 1,
            })
        };
        if self.complete {
            if io.input_closed(KernelPortId(0)) {
                return if io.consume_closed(KernelPortId(0)).is_ok() {
                    StepOutcome::Complete
                } else {
                    failure()
                };
            }
            return if io.input(KernelPortId(0)).is_some() {
                failure()
            } else {
                StepOutcome::Await
            };
        }
        if self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != RequestId(0)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
                || outcome.output.is_some()
                || io.consume_host_completion().is_err()
            {
                return failure();
            }
            self.pending = false;
            self.complete = true;
            self.acknowledged = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(KernelPortId(0)) {
            let Ok(value) = BoundedValueRef::new(value, source::MAXIMUM_INSTRUCTION_BYTES) else {
                return failure();
            };
            if io.consume(KernelPortId(0)).is_err()
                || io
                    .request_host_call(RequestId(0), HostCallId(0), value)
                    .is_err()
            {
                return failure();
            }
            self.pending = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
    fn cancel(&mut self) {
        self.complete = true;
        self.pending = false;
    }
}
