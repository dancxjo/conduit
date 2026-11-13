//! Shared USB Source execution through the existing production kernel.
//! Pure call owners are retained; physical endpoint calls require native admission.
use crate::{
    protocol_host_calls::ProtocolCallRefusal,
    protocol_source::PreparedProtocolArtifact,
    pure_protocol_owner::{PureProtocolOwner, PureProtocolPreparation},
};
use alloc::vec::Vec;
use conduit_composite::{KernelCompositeHost, KernelCompositeSignStorage, KernelOperationRegistry};
use conduit_core::*;
use conduit_kernel::{HostCallDisposition, HostCallOutcome, NodeId};
use conduit_plan_lowering::lowering::lower_plan_fragment;

pub struct PreparedUsbSourceKernel {
    kernel: KernelCompositeHost,
    pure: Vec<(NodeId, PureProtocolOwner)>,
    identity: PreparationHostIdentity,
}
/// Existing HID callers share this same USB kernel wrapper.
pub type PreparedHidSourceKernel = PreparedUsbSourceKernel;

impl PreparedUsbSourceKernel {
    pub fn prepare(
        artifact: PreparedProtocolArtifact,
        storage: KernelCompositeSignStorage,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_for(
            artifact,
            storage,
            Some(super::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION),
        )
    }

    /// Control transactions use the same prepared operations and production
    /// kernel. The caller must independently retain native control possession.
    pub fn prepare_control(
        artifact: PreparedProtocolArtifact,
        storage: KernelCompositeSignStorage,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_for(
            artifact,
            storage,
            Some(super::control_factory::CONTROL_IMPLEMENTATION),
        )
    }

    /// Pure descriptor/state preparation uses the same finite operations and
    /// kernel, with no native call factory or physical authority.
    pub fn prepare_pure(
        artifact: PreparedProtocolArtifact,
        storage: KernelCompositeSignStorage,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_for(artifact, storage, None)
    }

    /// Clock waits share the same kernel and independently admitted clock owner.
    pub(crate) fn prepare_clock(
        artifact: PreparedProtocolArtifact,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_for(
            artifact,
            KernelCompositeSignStorage {
                additional_local_items: 8192,
                additional_remote_items: 8192,
            },
            Some(crate::monotonic_clock::installation::CLOCK_IMPLEMENTATION),
        )
    }

    fn prepare_for(
        artifact: PreparedProtocolArtifact,
        storage: KernelCompositeSignStorage,
        native_implementation: Option<&str>,
    ) -> Result<Self, ProtocolCallRefusal> {
        use ProtocolCallRefusal as Error;
        let (definition, operations) = artifact.into_preparation_parts();
        crate::protocol_play::validate_fore(&definition)?;
        let plan = &definition.internal_plan;
        let fragment = &plan.fragments[0];
        let endpoint_count = fragment
            .placements
            .iter()
            .filter(|gear| Some(gear.implementation_id.as_str()) == native_implementation)
            .count();
        if (native_implementation.is_some() && !(1..=8).contains(&endpoint_count))
            || !plan.activations.is_empty()
            || !plan.activation_preparations.is_empty()
        {
            return Err(Error::Unsupported);
        }
        operations.validate_plan(plan).map_err(|error| {
            Error::Kernel(conduit_composite::KernelCompositeError::ChildRefused {
                child: fragment.host_id.clone(),
                reason: alloc::format!("{}: {}", error.family, error.reason),
            })
        })?;
        let lowered = lower_plan_fragment(fragment).map_err(|error| {
            Error::Kernel(conduit_composite::KernelCompositeError::Lowering {
                child: fragment.host_id.clone(),
                error,
            })
        })?;
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let identity = PreparationHostIdentity {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
        };
        let preparation = PureProtocolPreparation::new(fragment, &lowered, &active)?;
        let mut pure = Vec::with_capacity(fragment.placements.len());
        for gear in &fragment.placements {
            if let Some(owner) = preparation.prepare(gear)? {
                let node = lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or(Error::InvalidPlan)?;
                pure.push((node, owner));
            } else if !gear.host_calls.is_empty()
                && Some(gear.implementation_id.as_str()) != native_implementation
            {
                return Err(Error::Unsupported);
            }
        }
        // Owners retain their admitted values. The borrowed preparation view
        // is finished; retire its temporary routing graph before the kernel
        // creates its own lowering and operation storage.
        drop(lowered);
        let mut registry = KernelOperationRegistry::new();
        registry
            .install(crate::filter_host_call::FilterOperationFactory::default())
            .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        registry
            .install(crate::expression_host_call::ExpressionOperationFactory::default())
            .map_err(|_| Error::InvalidPlan)?;
        registry
            .install(crate::structured_selector_host_call::SelectorOperationFactory::default())
            .map_err(|_| Error::InvalidPlan)?;
        registry
            .install(crate::current_sample::CurrentSampleOperationFactory::default())
            .map_err(|_| Error::InvalidPlan)?;
        registry
            .install(operations.joins)
            .map_err(|_| Error::InvalidPlan)?;
        registry
            .install(operations.states)
            .map_err(|_| Error::InvalidPlan)?;
        registry
            .install(operations.concats)
            .map_err(|_| Error::InvalidPlan)?;
        registry
            .install(operations.merges)
            .map_err(|_| Error::InvalidPlan)?;
        if native_implementation == Some(super::control_factory::CONTROL_IMPLEMENTATION) {
            registry
                .install(
                    super::control_factory::ControlOperationFactory::prepare_contract()
                        .map_err(|_| Error::InvalidPlan)?,
                )
                .map_err(|_| Error::InvalidPlan)?;
        } else if native_implementation
            == Some(crate::monotonic_clock::installation::CLOCK_IMPLEMENTATION)
        {
            registry
                .install(
                    crate::monotonic_clock::factory::ClockOperationFactory::prepare_contract()
                        .map_err(|_| Error::InvalidPlan)?,
                )
                .map_err(|_| Error::InvalidPlan)?;
        } else if native_implementation.is_some() {
            registry
                .install(
                    super::endpoint_read_factory::EndpointReadOperationFactory::prepare_contract()
                        .map_err(|_| Error::InvalidPlan)?,
                )
                .map_err(|_| Error::InvalidPlan)?;
        }
        let kernel = KernelCompositeHost::prepare_with_sign_storage(definition, &registry, storage)
            .map_err(Error::Kernel)?;
        Ok(Self {
            kernel,
            pure,
            identity,
        })
    }

    /// The caller drives the existing kernel and separately binds its native
    /// owner to the exact planned endpoint. This access issues no possession.
    pub fn kernel_mut(&mut self) -> &mut KernelCompositeHost {
        &mut self.kernel
    }

    /// Service at most one already-issued pure call. A physical call remains
    /// pending for the native owner; no metadata grant substitutes for possession.
    pub fn service_pure_call(
        &mut self,
        request: &conduit_composite::KernelCompositeHostRequest,
    ) -> Result<bool, ProtocolCallRefusal> {
        use ProtocolCallRefusal as Error;
        let view = self
            .kernel
            .host_request_view(request)
            .map_err(Error::Kernel)?;
        let node = view.request.node;
        let Some((_, owner)) = self.pure.iter_mut().find(|(actual, _)| *actual == node) else {
            return Ok(false);
        };
        if *view.child != self.identity.host_id {
            return Err(Error::InvalidPlan);
        }
        let obligation = self
            .kernel
            .host_request_obligation(request)
            .map_err(Error::Kernel)?;
        if obligation.requirement.contract_id.as_str() != owner.contract() {
            return Err(Error::InvalidPlan);
        }
        let admitted = self
            .kernel
            .admit_host_request(request, &self.identity, &[], &[])
            .map_err(Error::Kernel)?;
        let call = *self
            .kernel
            .admitted_host_request_view(&admitted)
            .map_err(Error::Kernel)?
            .request;
        let input = self
            .kernel
            .host_request_input(&admitted)
            .map_err(Error::Kernel)?;
        match owner.invoke(node, call.call, call.request, input) {
            Ok(Some(bytes)) => self
                .kernel
                .complete_host_call_bytes(&admitted, bytes)
                .map_err(Error::Kernel)?,
            Ok(None) => self
                .kernel
                .complete_host_call(
                    &admitted,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .map_err(Error::Kernel)?,
            Err(error) => {
                let failure = error.failure();
                self.kernel
                    .complete_host_call(
                        &admitted,
                        HostCallOutcome {
                            disposition: if failure.code
                                == conduit_kernel::FailureCode::HostCallDenied
                            {
                                HostCallDisposition::Denied
                            } else {
                                HostCallDisposition::Failed
                            },
                            output: None,
                            failure: Some(failure),
                        },
                    )
                    .map_err(Error::Kernel)?;
                return Err(error);
            }
        }
        Ok(true)
    }
}
