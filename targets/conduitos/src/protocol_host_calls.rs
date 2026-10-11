//! Exact adapters for kernel-issued calls; protocol scheduling remains in the graph.
use crate::{
    expression_host_call,
    i2c_base::{
        I2cProvider,
        contract::{I2C_CALL, I2cContract},
        installation::{I2C_IMPLEMENTATION, ReadyI2cBase},
        owner::{I2cCallSelection, I2cHostCall},
    },
    pure_protocol_owner::PureProtocolOwner,
    structured_selector_host_call,
};
use alloc::{boxed::Box, vec::Vec};
use conduit_composite::{AdmittedKernelCompositeHostRequest, KernelCompositeHost};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId};
use conduit_plan_lowering::lowering::lower_plan_fragment;

pub use crate::protocol_call_refusal::ProtocolCallRefusal;

mod dispatch;
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
))]
mod usb_preparation;
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
))]
mod subordinate;


enum Owner<P> {
    Pure(PureProtocolOwner),
    I2c(Box<I2cHostCall<P>>),
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    UsbControl(Box<crate::arch::NativeUsbControlCall>),
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    UsbProtocolControl,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    UsbProtocolEndpoint,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    UsbDuplexControl,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    UsbDuplexRead,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    UsbDuplexWrite,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    UsbIsochronousControl,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    UsbIsochronousOut,
}
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
))]
#[derive(Clone, Copy)]
enum PendingUsbKind {
    ProtocolRead,
    DuplexRead,
    DuplexWrite,
    IsochronousOut,
}
struct Binding<P> {
    node: NodeId,
    resources: Vec<ResourceBinding>,
    authorities: Vec<AuthorityBinding>,
    owner: Owner<P>,
}
/// Admitted native owners and exact dispatch bindings prepared before Play.
pub struct PreparedProtocolCalls<P> {
    plan_id: PlanId,
    identity: PreparationHostIdentity,
    bindings: Vec<Binding<P>>,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    usb_protocol: Option<crate::arch::NativeUsbProtocolHost>,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    usb_duplex: Option<crate::arch::NativeUsbDuplexHost>,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    usb_isochronous: Option<crate::arch::NativeUsbIsochronousHost>,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pending_usb_kind: PendingUsbKind,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pending_usb: Option<AdmittedKernelCompositeHostRequest>,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pending_usb_failed: bool,
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pending_duplex: [Option<(conduit_kernel::NodeId, AdmittedKernelCompositeHostRequest)>; 2],
}
impl<P: I2cProvider> PreparedProtocolCalls<P> {
    pub fn prepare(
        plan: &Plan,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_with_joins(
            plan,
            ready,
            table,
            handle,
            claim,
            &crate::flow_zip::FlowZipOperationFactory::default(),
        )
    }

    pub(crate) fn prepare_with_joins(
        plan: &Plan,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        zip: &crate::flow_zip::FlowZipOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_with_operations(
            plan,
            ready,
            table,
            handle,
            claim,
            zip,
            &crate::seeded_state::SeededStateOperationFactory::default(),
        )
    }

    pub(crate) fn prepare_with_operations(
        plan: &Plan,
        ready: ReadyI2cBase<P>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_selected(
            plan,
            I2C_IMPLEMENTATION,
            zip,
            states,
            |fragment, lowered, active, placement| {
                let contract =
                    I2cContract::prepare().map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
                ready
                    .bind_selected(
                        table,
                        handle,
                        claim,
                        I2cCallSelection {
                            contract: &contract,
                            fragment,
                            lowered,
                            active,
                            placement,
                        },
                    )
                    .map(|owner| Owner::I2c(Box::new(owner)))
                    .map_err(ProtocolCallRefusal::I2c)
            },
        )
    }

    fn prepare_pure(
        plan: &Plan,
        implementations: &[&str],
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<
        (
            conduit_plan_lowering::lowering::LoweredPlanFragment,
            ActivePlayIdentity,
            Vec<Binding<P>>,
        ),
        ProtocolCallRefusal,
    > {
        use ProtocolCallRefusal as Refusal;
        if !verify_plan(plan) || plan.fragments.len() != 1 {
            return Err(Refusal::InvalidPlan);
        }
        states
            .validate_plan(plan)
            .map_err(|_| Refusal::InvalidPlan)?;
        let fragment = &plan.fragments[0];
        let lowered = lower_plan_fragment(fragment).map_err(|_| Refusal::InvalidPlan)?;
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        for implementation in implementations {
            if fragment
                .placements
                .iter()
                .filter(|gear| gear.implementation_id.as_str() == *implementation)
                .count()
                != 1
            {
                return Err(Refusal::Unsupported);
            }
        }
        if fragment.placements.iter().any(|gear| {
            !implementations.contains(&gear.implementation_id.as_str())
                && !matches!(
                    gear.implementation_id.as_str(),
                    expression_host_call::IMPLEMENTATION
                        | crate::offer::TEXT_LITERAL_IMPLEMENTATION
                        | crate::flow_singleton::IMPLEMENTATION
                        | structured_selector_host_call::IMPLEMENTATION
                        | crate::flow_zip::IMPLEMENTATION
                        | crate::flow_concat_finite::IMPLEMENTATION
                        | crate::flow_merge_finite::IMPLEMENTATION
                        | crate::seeded_state::IMPLEMENTATION
                        | crate::current_sample::IMPLEMENTATION
                        | crate::monotonic_clock::installation::CLOCK_IMPLEMENTATION
                )
        }) {
            return Err(Refusal::Unsupported);
        }
        // Validate pure typed storage before binding the physical owner.
        zip.validate_plan(plan).map_err(|_| Refusal::InvalidPlan)?;
        for gear in &fragment.placements {
            if gear.implementation_id.as_str() == crate::current_sample::IMPLEMENTATION {
                use conduit_composite::KernelOperationFactory;
                crate::current_sample::CurrentSampleOperationFactory::default()
                    .budget(gear)
                    .map_err(|_| Refusal::InvalidPlan)?;
            }
        }
        let mut bindings = Vec::with_capacity(fragment.placements.len());
        for gear in &fragment.placements {
            let Some(owner) = PureProtocolOwner::prepare(fragment, &lowered, &active, gear)? else {
                continue;
            };
            bindings.push(Binding {
                node: lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or(Refusal::InvalidPlan)?,
                resources: gear.resources.clone(),
                authorities: gear.authority.clone(),
                owner: Owner::Pure(owner),
            });
        }
        Ok((lowered, active, bindings))
    }

    fn prepare_selected(
        plan: &Plan,
        implementation: &str,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
        bind: impl FnOnce(
            &PlanFragment,
            &conduit_plan_lowering::lowering::LoweredPlanFragment,
            &ActivePlayIdentity,
            &PlacementId,
        ) -> Result<Owner<P>, ProtocolCallRefusal>,
    ) -> Result<Self, ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        let (lowered, active, mut bindings) =
            Self::prepare_pure(plan, &[implementation], zip, states)?;
        let fragment = &plan.fragments[0];
        let i2c = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == implementation)
            .ok_or(Refusal::Unsupported)?;
        let owner = bind(fragment, &lowered, &active, &i2c.placement_id)?;
        bindings.push(Binding {
            node: lowered
                .identity
                .node_for_placement(&i2c.placement_id)
                .ok_or(Refusal::InvalidPlan)?,
            resources: i2c.resources.clone(),
            authorities: i2c.authority.clone(),
            owner,
        });
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            identity: PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            bindings,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            usb_protocol: None,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            usb_duplex: None,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            usb_isochronous: None,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb_kind: PendingUsbKind::ProtocolRead,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb: None,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb_failed: false,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_duplex: [None, None],
        })
    }

    /// Revoke actual bus ownership as well as cancelling kernel and computation state.
    pub fn cancel(&mut self, kernel: &mut KernelCompositeHost) -> Result<(), ProtocolCallRefusal> {
        use ProtocolCallRefusal as Refusal;
        if kernel.definition().internal_plan.plan_id != self.plan_id {
            return Err(Refusal::InvalidPlan);
        }
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
        ))]
        if let Some(host) = &mut self.usb_protocol {
            host.cancel().map_err(Refusal::UsbProtocol)?;
            self.pending_duplex = [None, None];
            self.pending_usb = None;
            self.pending_usb_failed = false;
        }
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
        ))]
        if let Some(host) = &mut self.usb_duplex {
            host.cancel().map_err(Refusal::UsbProtocol)?;
            self.pending_duplex = [None, None];
            self.pending_usb = None;
            self.pending_usb_failed = false;
        }
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
        ))]
        if let Some(host) = &mut self.usb_isochronous {
            host.cancel().map_err(Refusal::UsbIsochronous)?;
            self.pending_usb = None;
            self.pending_usb_failed = false;
        }
        // Revoke the physics-facing owner first, even if kernel cancellation later refuses.
        for binding in &mut self.bindings {
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            if let Owner::UsbControl(owner) = &mut binding.owner {
                owner
                    .revoke(binding.node, HostCallId(0))
                    .map_err(Refusal::Usb)?;
            }
            if let Owner::I2c(owner) = &mut binding.owner {
                owner
                    .revoke(binding.node, HostCallId(0))
                    .map_err(Refusal::I2c)?;
            }
        }
        let cancelled = kernel.cancel().map_err(Refusal::Kernel);
        for binding in &mut self.bindings {
            if let Owner::Pure(owner) = &mut binding.owner {
                owner.cancel(binding.node, HostCallId(0))?;
            }
        }
        cancelled
    }
}
