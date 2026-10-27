//! Owned cooperative renderer: an exact admitted Face Host Call produces a Show
//! only after the selected electrical provider acknowledges its finite packets.
//! Body/current-route admission and the outer kernel remain Root responsibilities.
use crate::{display_mask::PreparedDisplayMask, display_packets::PreparedDisplayPackets};
use alloc::vec;
use conduit_core as c;
use conduit_face as f;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::{
    selected_assigned_operation::{
        bind_selected_assigned_configured_fore_operation, AssignedNativeIdentity,
        SelectedAssignedForeOperationPlan,
    },
    selected_operation::SelectedOperationContract,
};
use conduit_tm1637_wire::{installation::WriteNativeIdentity, OpenDrain, Packet};

pub const CAPABILITY: &str = "conduit/seven-segment-renderer@1";
pub const CALL: &str = "conduit/seven-segment-present@1";
pub const PROFILE: &str = "conduit/seven-segment-cooperative@1";
pub const ATTACHMENT: &str = "machine/tm1637-attachment";
pub const AUTHORITY: &str = "conduit.authority/seven-segment-present@1";
/// Electrical identity fields only. No WRITE Kind, grant or capability is reused.
pub type RendererIdentity = WriteNativeIdentity;
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub face_bytes: u32,
    pub show_bytes: u32,
    /// Required protected attachment bytes. Kernel, immutable program material
    /// and Show staging storage need separate admission; this is not total RAM.
    pub resource_bytes: u64,
}
pub struct ReadyRenderer<P> {
    identity: RendererIdentity,
    bounds: Bounds,
    provider: P,
    mask: PreparedDisplayMask,
    packets: PreparedDisplayPackets,
    next_request: u32,
}
#[path = "mask_renderer_delivery.rs"]
mod delivery;
#[path = "mask_renderer_invocation.rs"]
mod invocation;
#[path = "mask_renderer_offer.rs"]
mod offer;
pub use delivery::{
    DeliveryRefusal, PreparedPresentation, PresentationBasis, RendererCall, RendererState,
    VerifiedPresentation,
};
pub use invocation::PreparedRendererInvocation;
#[cfg(feature = "local-planning")]
pub use invocation::{DirectRendererSelection, RendererSourceConfiguration};
impl<P: OpenDrain> ReadyRenderer<P> {
    /// # Safety
    /// Root must transfer the exact initialized pins named by identity, retain
    /// their exclusive resource/attachment generation and admit finite storage,
    /// cooperative transaction/cleanup work and timing. The offered artifact
    /// and implementation must identify this exact immutable Mask/Look and
    /// checked packet material; received Plans cannot choose those facts.
    /// Provider Drop releases
    /// pins. This does no electrical work and does not admit a Body route/Play.
    pub unsafe fn new(
        identity: RendererIdentity,
        bounds: Bounds,
        provider: P,
        mask: PreparedDisplayMask,
    ) -> Result<Self, &'static str> {
        if bounds.face_bytes == 0
            || bounds.face_bytes > 8192
            || bounds.show_bytes == 0
            || bounds.show_bytes > 8192
            || bounds.resource_bytes < 24
            || identity.provider_generation == 0
            || [
                identity.host_id.as_str(),
                identity.boot_id.as_str(),
                identity.base_id.as_str(),
                identity.provider_instance_id.as_str(),
                identity.resource_pool_id.as_str(),
                &identity.resource_generation_id.0,
                identity.envelope_id.as_str(),
                identity.artifact_id.as_str(),
                identity.implementation_id.as_str(),
                identity.base_implementation_id.as_str(),
            ]
            .iter()
            .any(|s| s.is_empty())
        {
            return Err("renderer-identity-or-bounds");
        }
        let packets = PreparedDisplayPackets::prepare().map_err(|_| "renderer-packets")?;
        Ok(Self {
            identity,
            bounds,
            provider,
            mask,
            packets,
            next_request: 0,
        })
    }
    fn realization_contract(&self) -> c::Kind {
        let mut kind = f::renderer_kind();
        // Exact finite offer limits, independently retained by this provider.
        kind.limits = c::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: self.bounds.face_bytes.max(self.bounds.show_bytes),
        };
        kind
    }
    /// Absolute request continuation retained with the actual electrical owner.
    pub fn request_sequence(&self) -> conduit_kernel::RequestId {
        conduit_kernel::RequestId(self.next_request)
    }
    pub fn identity(&self) -> &RendererIdentity {
        &self.identity
    }
    /// All fallible checks precede transfer of the actual initialized provider.
    /// Expected configuration is independently retained checked Source material.
    pub fn bind_assigned_retained(
        owner: &mut Option<Self>,
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
        configuration: &[c::ConfigurationEntry],
        selected: SelectedAssignedForeOperationPlan<'_>,
    ) -> Result<RendererCall<P>, &'static str> {
        let ready = owner.as_ref().ok_or("renderer-unavailable")?;
        let identity = &ready.identity;
        let b = ready.bounds;
        if claim.host_id != identity.host_id
            || claim.boot_id != identity.boot_id
            || claim.base_instance_id != identity.provider_instance_id
            || claim.base_provider_generation != identity.provider_generation
            || claim.resource_pool_id != identity.resource_pool_id
            || claim.resource_generation_id != identity.resource_generation_id
            || claim.envelope_id != identity.envelope_id
            || claim.implementation_id != identity.implementation_id
            || claim.parameter_bytes != b.face_bytes
            || claim.work_units != 1
        {
            return Err("renderer-native-identity");
        }
        let mut inspections = table.inspections();
        let scope = &inspections.next().ok_or("renderer-possession")?.scope;
        // One presentation and, if acknowledged, one explicit withdrawal.
        if inspections.next().is_some()
            || scope.maximum_in_flight != 1
            || scope.maximum_parameter_bytes != b.face_bytes
            || scope.maximum_result_bytes != b.show_bytes
            || scope.maximum_work_units != 1
            || scope.maximum_operations != 2
        {
            return Err("renderer-possession-bound");
        }
        drop(inspections);
        let active = selected.operation.active.clone();
        let node = bind_selected_assigned_configured_fore_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: &ready.realization_contract(),
                call: CALL,
                input_bytes: b.face_bytes,
                output_bytes: b.show_bytes,
                resource_bytes: b.resource_bytes,
            },
            configuration,
            AssignedNativeIdentity {
                capability: CAPABILITY,
                execution_profile: PROFILE,
                artifact: identity.artifact_id.as_str(),
                base: identity.base_id.as_str(),
                base_implementation: identity.base_implementation_id.as_str(),
                mechanism: ATTACHMENT,
                authority: AUTHORITY,
                resource_class: ATTACHMENT,
            },
            selected,
        )
        .map_err(|_| "renderer-selected-binding")?;
        let ready = owner.take().expect("preflighted renderer owner");
        Ok(RendererCall {
            ready,
            table,
            handle,
            claim,
            active,
            node,
            state: RendererState::Ready,
        })
    }
    /// Bind actual pins to exact locally verified Mask selection. Fore pins and
    /// configuration come from independently retained checked Source. Root must
    /// separately admit the current Body/Face/Mask route and held-start release.
    pub fn bind_verified_retained(
        owner: &mut Option<Self>,
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
        configuration: &[c::ConfigurationEntry],
        selected: conduit_plan_lowering::selected_operation::SelectedVerifiedForeOperationPlan<'_>,
    ) -> Result<RendererCall<P>, &'static str> {
        let ready = owner.as_ref().ok_or("renderer-unavailable")?;
        let active = selected.operation.active;
        let node = Self::check_verified(ready, &table, &claim, configuration, selected)?;
        let active = active.clone();
        let ready = owner.take().expect("preflighted renderer owner");
        Ok(RendererCall {
            ready,
            table,
            handle,
            claim,
            active,
            node,
            state: RendererState::Ready,
        })
    }
    fn check_verified(
        ready: &Self,
        table: &c::BaseCapabilityTable,
        claim: &c::BaseOperationClaim,
        configuration: &[c::ConfigurationEntry],
        selected: conduit_plan_lowering::selected_operation::SelectedVerifiedForeOperationPlan<'_>,
    ) -> Result<NodeId, &'static str> {
        Self::check_possession(ready, table, claim)?;
        let identity = &ready.identity;
        let b = ready.bounds;
        let node = conduit_plan_lowering::selected_operation::bind_selected_verified_configured_fore_operation(
            table, claim,
            SelectedOperationContract {
                kind: &ready.realization_contract(), call: CALL,
                input_bytes: b.face_bytes, output_bytes: b.show_bytes,
                resource_bytes: b.resource_bytes,
            }, configuration,
            AssignedNativeIdentity {
                capability: CAPABILITY, execution_profile: PROFILE,
                artifact: identity.artifact_id.as_str(), base: identity.base_id.as_str(),
                base_implementation: identity.base_implementation_id.as_str(),
                mechanism: ATTACHMENT, authority: AUTHORITY, resource_class: ATTACHMENT,
            }, selected,
        ).map_err(|_| "renderer-selected-binding")?;
        Ok(node)
    }
    fn check_possession(
        ready: &Self,
        table: &c::BaseCapabilityTable,
        claim: &c::BaseOperationClaim,
    ) -> Result<(), &'static str> {
        let identity = &ready.identity;
        let b = ready.bounds;
        if claim.host_id != identity.host_id
            || claim.boot_id != identity.boot_id
            || claim.base_instance_id != identity.provider_instance_id
            || claim.base_provider_generation != identity.provider_generation
            || claim.resource_pool_id != identity.resource_pool_id
            || claim.resource_generation_id != identity.resource_generation_id
            || claim.envelope_id != identity.envelope_id
            || claim.implementation_id != identity.implementation_id
            || claim.parameter_bytes != b.face_bytes
            || claim.work_units != 1
        {
            return Err("renderer-native-identity");
        }
        let mut inspections = table.inspections();
        let scope = &inspections.next().ok_or("renderer-possession")?.scope;
        // One presentation and, if acknowledged, one explicit withdrawal.
        if inspections.next().is_some()
            || scope.maximum_in_flight != 1
            || scope.maximum_parameter_bytes != b.face_bytes
            || scope.maximum_result_bytes != b.show_bytes
            || scope.maximum_work_units != 1
            || scope.maximum_operations != 2
        {
            return Err("renderer-possession-bound");
        }
        drop(inspections);
        Ok(())
    }
}
