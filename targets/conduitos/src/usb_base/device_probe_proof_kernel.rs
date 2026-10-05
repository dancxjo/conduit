//! One admitted descriptor proof kernel with retained pure call owners.
//! Native transfer possession is supplied separately by the trusted proof root.
use super::{
    control_factory::{CONTROL_IMPLEMENTATION, ControlOperationFactory},
    control_proof_plan::ControlProofSubject,
    device_probe_proof_plan::{self, DeviceProbeProofRefusal},
};
use crate::{
    expression_host_call::{
        self, ExpressionCallRefusal, ExpressionHostCall, ExpressionOperationFactory,
    },
    structured_selector_host_call::{
        self, SelectorCallRefusal, SelectorHostCall, SelectorOperationFactory,
    },
};
use alloc::vec::Vec;
use conduit_composite::{
    KernelCompositeError, KernelCompositeHost, KernelCompositeHostRequest,
    KernelCompositeSignStorage, KernelOperationRegistry,
};
use conduit_core::{Plan, bind_active_play};
use conduit_kernel::{HostCallDisposition, HostCallOutcome, NodeId};
use conduit_plan_lowering::lowering::lower_plan_fragment;

#[derive(Debug)]
pub enum DeviceProbeKernelRefusal {
    Plan(DeviceProbeProofRefusal),
    Kernel(KernelCompositeError),
    Expression(ExpressionCallRefusal),
    Selector(SelectorCallRefusal),
    InvalidBinding,
}
enum PureOwner {
    Expression(ExpressionHostCall),
    Selector(SelectorHostCall),
}

pub struct PreparedDeviceProbeKernel {
    pub kernel: KernelCompositeHost,
    pure: Vec<(NodeId, PureOwner)>,
}
impl PreparedDeviceProbeKernel {
    pub fn prepare(
        subject: &ControlProofSubject<'_>,
        storage: KernelCompositeSignStorage,
    ) -> Result<Self, DeviceProbeKernelRefusal> {
        use DeviceProbeKernelRefusal as Error;
        let artifact = device_probe_proof_plan::prepare(subject).map_err(Error::Plan)?;
        Self::from_artifact(artifact, storage)
    }

    /// Configuration descriptors use the same one-control-call proof appliance.
    pub fn prepare_configuration(
        subject: &ControlProofSubject<'_>,
        storage: KernelCompositeSignStorage,
    ) -> Result<Self, DeviceProbeKernelRefusal> {
        let artifact = super::configuration_probe_proof_plan::prepare(subject)
            .map_err(DeviceProbeKernelRefusal::Plan)?;
        Self::from_artifact(artifact, storage)
    }

    fn from_artifact(
        artifact: crate::protocol_source::PreparedProtocolArtifact,
        storage: KernelCompositeSignStorage,
    ) -> Result<Self, DeviceProbeKernelRefusal> {
        use DeviceProbeKernelRefusal as Error;
        let definition = artifact.artifact().definition().clone();
        let fragment = &definition.internal_plan.fragments[0];
        let lowered = lower_plan_fragment(fragment).map_err(|_| Error::InvalidBinding)?;
        let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let mut pure = Vec::new();
        let mut transfers = 0;
        for placement in &fragment.placements {
            let node = lowered
                .identity
                .placements
                .iter()
                .find(|(_, id)| id == &placement.placement_id)
                .ok_or(Error::InvalidBinding)?
                .0;
            let owner = match placement.implementation_id.as_str() {
                expression_host_call::IMPLEMENTATION => Some(PureOwner::Expression(
                    ExpressionHostCall::prepare(
                        fragment,
                        &lowered,
                        &active,
                        &placement.placement_id,
                    )
                    .map_err(Error::Expression)?,
                )),
                structured_selector_host_call::IMPLEMENTATION => Some(PureOwner::Selector(
                    SelectorHostCall::prepare(fragment, &lowered, &active, &placement.placement_id)
                        .map_err(Error::Selector)?,
                )),
                CONTROL_IMPLEMENTATION => {
                    transfers += 1;
                    None
                }
                _ => return Err(Error::InvalidBinding),
            };
            if let Some(owner) = owner {
                pure.push((node, owner));
            }
        }
        if transfers != 1 {
            return Err(Error::InvalidBinding);
        }
        let mut registry = KernelOperationRegistry::new();
        registry
            .install(ExpressionOperationFactory::default())
            .map_err(|_| Error::InvalidBinding)?;
        registry
            .install(SelectorOperationFactory::default())
            .map_err(|_| Error::InvalidBinding)?;
        registry
            .install(
                ControlOperationFactory::prepare_contract().map_err(|_| Error::InvalidBinding)?,
            )
            .map_err(|_| Error::InvalidBinding)?;
        let kernel = KernelCompositeHost::prepare_with_sign_storage(definition, &registry, storage)
            .map_err(Error::Kernel)?;
        Ok(Self { kernel, pure })
    }

    pub fn plan(&self) -> &Plan {
        &self.kernel.definition().internal_plan
    }

    /// Dispatch only already-prepared pure owners. False leaves a native request
    /// unadmitted so its independent physical owner can validate it exactly.
    pub fn dispatch_pure(
        &mut self,
        request: &KernelCompositeHostRequest,
    ) -> Result<bool, DeviceProbeKernelRefusal> {
        use DeviceProbeKernelRefusal as Error;
        let view = self
            .kernel
            .host_request_view(request)
            .map_err(Error::Kernel)?;
        let node = view.request.node;
        let Some((_, owner)) = self.pure.iter_mut().find(|(actual, _)| *actual == node) else {
            return Ok(false);
        };
        let obligation = self
            .kernel
            .host_request_obligation(request)
            .map_err(Error::Kernel)?;
        let expected = match owner {
            PureOwner::Expression(_) => expression_host_call::CALL,
            PureOwner::Selector(_) => structured_selector_host_call::CALL,
        };
        if obligation.requirement.contract_id.as_str() != expected {
            return Err(Error::InvalidBinding);
        }
        let admitted = self
            .kernel
            .admit_host_request(
                request,
                &obligation.host,
                &obligation.resources,
                &obligation.authorities,
            )
            .map_err(Error::Kernel)?;
        let call = *self
            .kernel
            .admitted_host_request_view(&admitted)
            .map_err(Error::Kernel)?
            .request;
        let bytes = self
            .kernel
            .host_request_input(&admitted)
            .map_err(Error::Kernel)?;
        let result = match owner {
            PureOwner::Expression(owner) => Some(
                owner
                    .invoke(node, call.call, call.request, bytes)
                    .map_err(Error::Expression)?,
            ),
            PureOwner::Selector(owner) => owner
                .invoke(node, call.call, call.request, bytes)
                .map_err(Error::Selector)?,
        };
        match result {
            Some(bytes) => self.kernel.complete_host_call_bytes(&admitted, bytes),
            None => self.kernel.complete_host_call(
                &admitted,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            ),
        }
        .map_err(Error::Kernel)?;
        Ok(true)
    }
}
