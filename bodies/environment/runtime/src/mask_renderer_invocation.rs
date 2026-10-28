//! Immutable selected-operation provenance survives Plan retirement. It grants
//! no native possession and cannot be constructed from descriptors or IDs.
use super::*;
use alloc::vec::Vec;
use conduit_plan_lowering::{
    lowering::{LoweredHostCall, LoweredNode},
    selected_operation::SelectedVerifiedForeOperationPlan,
};
/// Cold inspection material. Consuming this wrapper retires rich descriptions.
pub struct PreparedRendererInvocation {
    execution: PreparedRendererExecution,
    node: LoweredNode,
    call: LoweredHostCall,
    configuration: Vec<c::ConfigurationEntry>,
    fore: Vec<c::PlannedForePort>,
}
#[path = "mask_renderer_fingerprint.rs"]
mod fingerprint;
/// Checked comparison provenance, never a descriptor-derived native grant.
/// Actual Ready/capability owners remain mandatory; fixed fingerprints retire
/// duplicated immutable metadata while preserving exact scope and lifecycle.
pub struct PreparedRendererExecution {
    owner: [u8; 32],
    scope: [u8; 32],
    parent: [u8; 32],
    minimum_sequence: u64,
    last: Option<LastPlay>,
    pub(super) bounds: Bounds,
    pub(super) bound: bool,
    pub(super) node: NodeId,
}
struct LastPlay {
    fingerprint: [u8; 32],
    sequence: u64,
}

impl core::ops::Deref for PreparedRendererInvocation {
    type Target = PreparedRendererExecution;
    fn deref(&self) -> &Self::Target {
        &self.execution
    }
}
impl core::ops::DerefMut for PreparedRendererInvocation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.execution
    }
}

impl<P: OpenDrain> ReadyRenderer<P> {
    /// All independent Source/Fore/native/ordinary numeric checks run against
    /// the live Core-verified Plan before creating this opaque, owned capsule.
    /// Root separately admits the unchanged parent BodyPlay and child horizon.
    pub fn prepare_verified_invocation(
        &self,
        table: &c::BaseCapabilityTable,
        claim: &c::BaseOperationClaim,
        configuration: &[c::ConfigurationEntry],
        selected: SelectedVerifiedForeOperationPlan<'_>,
    ) -> Result<PreparedRendererInvocation, &'static str> {
        let initial = selected.operation.active;
        let lowered = selected.operation.lowered;
        let fore = selected.fore;
        let node = Self::check_verified(self, table, claim, configuration, selected)?;
        let inspection = table.inspections().next().ok_or("renderer-possession")?;
        if inspection.completed_operations != 0 {
            return Err("renderer-invocation-spent");
        }
        Ok(PreparedRendererInvocation {
            execution: PreparedRendererExecution::checked(
                &self.identity,
                self.bounds,
                &inspection.scope,
                initial,
                node,
            ),
            node: lowered
                .nodes
                .iter()
                .find(|n| n.node == node)
                .ok_or("renderer-node")?
                .clone(),
            call: lowered
                .host_calls
                .iter()
                .find(|c| c.node == node)
                .ok_or("renderer-call")?
                .clone(),
            configuration: configuration.to_vec(),
            fore: fore.to_vec(),
        })
    }
}
impl PreparedRendererInvocation {
    pub fn node(&self) -> NodeId {
        self.node.node
    }
    pub fn configuration(&self) -> &[c::ConfigurationEntry] {
        &self.configuration
    }
    pub fn fore(&self) -> &[c::PlannedForePort] {
        &self.fore
    }
    pub fn numeric_node(&self) -> &LoweredNode {
        &self.node
    }
    pub fn numeric_call(&self) -> &LoweredHostCall {
        &self.call
    }
    /// Retire complete cold descriptions after all checked construction.
    #[cold]
    #[inline(never)]
    pub fn into_execution(self) -> PreparedRendererExecution {
        self.execution
    }
}
impl PreparedRendererExecution {
    // This constructor stays private to the verified preparation module. Hashes
    // cannot be supplied by a caller or used to issue native possession.
    fn checked(
        identity: &RendererIdentity,
        bounds: Bounds,
        scope: &c::BaseCapabilityScope,
        initial: &c::ActivePlayIdentity,
        node: NodeId,
    ) -> Self {
        Self {
            owner: fingerprint::owner(identity, bounds, node),
            scope: fingerprint::scope(scope),
            parent: fingerprint::parent(initial),
            minimum_sequence: initial.play_sequence,
            last: None,
            bounds,
            bound: false,
            node,
        }
    }
    pub(super) fn matches_owner<P: OpenDrain>(&self, ready: &ReadyRenderer<P>) -> bool {
        ready.bounds == self.bounds
            && fingerprint::owner(&ready.identity, ready.bounds, self.node) == self.owner
    }
    pub(super) fn matches_parent(&self, active: &c::ActivePlayIdentity) -> bool {
        fingerprint::parent(active) == self.parent
    }
    pub(super) fn matches_scope(&self, scope: &c::BaseCapabilityScope) -> bool {
        fingerprint::scope(scope) == self.scope
    }
    pub(super) fn matches_last(&self, active: &c::ActivePlayIdentity) -> bool {
        self.last.as_ref().is_some_and(|last| {
            last.sequence == active.play_sequence && last.fingerprint == fingerprint::active(active)
        })
    }
    pub(super) fn record_last(&mut self, active: &c::ActivePlayIdentity) {
        self.last = Some(LastPlay {
            fingerprint: fingerprint::active(active),
            sequence: active.play_sequence,
        });
    }
    pub fn node(&self) -> NodeId {
        self.node
    }
    /// A fresh Root-issued two-operation lease remains mandatory. The exact
    /// physical owner and resource generation remain stable within the parent
    /// BodyPlay. No request counter or finite Look bound is reset here.
    #[allow(clippy::too_many_arguments)]
    pub fn bind_retained<P: OpenDrain>(
        &mut self,
        owner: &mut Option<ReadyRenderer<P>>,
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
        active: &c::ActivePlayIdentity,
    ) -> Result<RendererCall<P>, &'static str> {
        let ready = owner.as_ref().ok_or("renderer-unavailable")?;
        // The renderer contract is immutable module material plus limits derived
        // only from Bounds. Exact identity and Bounds equality therefore retain
        // the same checked contract without an owned duplicate Kind.
        if self.bound || !self.matches_owner(ready) {
            return Err("renderer-invocation-owner");
        }
        if !self.matches_parent(active)
            || active.play_sequence < self.minimum_sequence
            || self
                .last
                .as_ref()
                .is_some_and(|last| active.play_sequence <= last.sequence)
            || c::bind_active_play(
                &active.plan_id,
                &active.host_id,
                &active.boot_id,
                active.play_sequence,
            ) != *active
        {
            return Err("renderer-invocation-play");
        }
        let mut inspections = table.inspections();
        let current = inspections.next().ok_or("renderer-possession")?;
        if inspections.next().is_some()
            || current.lifecycle != c::CapabilityLifecycle::Issued
            || current.in_flight_operations != 0
            || current.completed_operations != 0
        {
            return Err("renderer-invocation-possession");
        }
        let scope = &current.scope;
        // Compare every originally verified scope field; only the independently
        // issued child ActivePlay changes. No new grants/native metadata enter.
        if !self.matches_scope(scope)
            || scope.active_play_id != active.active_play_id
            || claim.host_id != scope.host_id
            || claim.boot_id != scope.boot_id
            || claim.plan_id != scope.plan_id
            || claim.active_play_id != scope.active_play_id
            || claim.base_instance_id != scope.base_instance_id
            || claim.base_provider_generation != scope.base_provider_generation
            || claim.implementation_id != scope.implementation_id
            || claim.operation_contract_id != scope.operation_contract_id
            || claim.subject_kind != scope.subject_kind
            || claim.resource_pool_id != scope.resource_pool_id
            || claim.resource_generation_id != scope.resource_generation_id
            || claim.envelope_id != scope.envelope_id
            || claim.parameter_bytes != self.bounds.face_bytes
            || claim.work_units != 1
        {
            return Err("renderer-invocation-scope");
        }
        drop(inspections);
        let active = active.clone();
        let ready = owner.take().expect("preflighted renderer");
        self.record_last(&active);
        self.bound = true;
        Ok(RendererCall {
            ready,
            table,
            handle,
            claim,
            active,
            node: self.node(),
            state: RendererState::Ready,
        })
    }
    /// Only this capsule can complete its invocation lifecycle. Generic owner
    /// retirement deliberately leaves it bound, preventing subsequent reuse.
    pub fn retire_invocation<P: OpenDrain>(
        &mut self,
        call: &mut Option<RendererCall<P>>,
    ) -> Result<ReadyRenderer<P>, &'static str> {
        let current = call.as_ref().ok_or("renderer-unavailable")?;
        if !self.bound
            || !self.matches_last(&current.active)
            || !self.matches_owner(&current.ready)
            || current.node != self.node()
        {
            return Err("renderer-invocation-retirement");
        }
        let ready = RendererCall::retire_retained(call)?;
        self.bound = false;
        Ok(ready)
    }
}
/// Independently checked linked Source configuration. Private fields prevent a
/// claimed Source/placement ID from becoming expected executable configuration.
#[cfg(feature = "local-planning")]
pub struct RendererSourceConfiguration {
    source: c::PlotIdentity,
    gear: c::GearId,
    entries: Vec<c::ConfigurationEntry>,
}
#[cfg(feature = "local-planning")]
impl RendererSourceConfiguration {
    pub fn prepare(
        image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        expected: &c::PlotIdentity,
        gear: &c::GearId,
    ) -> Result<Self, &'static str> {
        if image.identity().map_err(|_| "renderer-source")? != *expected {
            return Err("renderer-source");
        }
        let mut selected = None;
        for index in 0..image.gear_count() {
            let (id, kind) = image.gear_identity(index).map_err(|_| "renderer-source")?;
            if &id != gear {
                continue;
            }
            if kind.as_str() != f::RENDERER_KIND {
                return Err("renderer-source-kind");
            }
            let configuration = image
                .gear_configuration(index)
                .map_err(|_| "renderer-source")?;
            if configuration.gear_id != id || selected.replace(configuration).is_some() {
                return Err("renderer-source-gear");
            }
        }
        let selected = selected.ok_or("renderer-source-gear")?;
        if selected.configuration.len() != 1 || c::face_data_label(&selected.configuration).is_err()
        {
            return Err("renderer-source-configuration");
        }
        Ok(Self {
            source: expected.clone(),
            gear: gear.clone(),
            entries: selected.configuration,
        })
    }
}
#[cfg(feature = "local-planning")]
pub struct DirectRendererSelection<'a> {
    pub plan: &'a c::VerifiedPlanMetadata,
    pub fragment_id: &'a c::FragmentId,
    pub active: &'a c::ActivePlayIdentity,
    pub placement_id: &'a c::PlacementId,
    pub fore: &'a [c::PlannedForePort],
}
#[cfg(feature = "local-planning")]
impl<P: OpenDrain> ReadyRenderer<P> {
    pub fn prepare_direct_invocation(
        &self,
        table: &c::BaseCapabilityTable,
        claim: &c::BaseOperationClaim,
        configuration: &RendererSourceConfiguration,
        selected: DirectRendererSelection<'_>,
    ) -> Result<PreparedRendererInvocation, &'static str> {
        Self::check_possession(self, table, claim)?;
        let fragment = selected
            .plan
            .fragment(selected.fragment_id)
            .ok_or("renderer-selected-binding")?;
        if !fragment
            .lowering_metadata()
            .placements
            .iter()
            .any(|g| &g.placement_id == selected.placement_id && g.gear_id == configuration.gear)
        {
            return Err("renderer-selected-binding");
        }
        let projection = conduit_plan_lowering::lowering::VerifiedForeOperationProjection::prepare(
            selected.plan,
            selected.fragment_id,
            &configuration.source,
            selected.fore,
            selected.placement_id,
        )
        .map_err(|_| "renderer-selected-binding")?;
        let identity = &self.identity;
        let b = self.bounds;
        conduit_plan_lowering::selected_operation::bind_selected_projected_configured_fore_operation(
            table,claim,
            SelectedOperationContract{kind:&self.realization_contract(),call:CALL,input_bytes:b.face_bytes,output_bytes:b.show_bytes,resource_bytes:b.resource_bytes},
            &configuration.entries,
            AssignedNativeIdentity{capability:CAPABILITY,execution_profile:PROFILE,artifact:identity.artifact_id.as_str(),base:identity.base_id.as_str(),base_implementation:identity.base_implementation_id.as_str(),mechanism:ATTACHMENT,authority:AUTHORITY,resource_class:ATTACHMENT},
            conduit_plan_lowering::selected_operation::SelectedProjectedForeOperationPlan{projection:&projection,active:selected.active},
        ).map_err(|_|"renderer-selected-binding")?;
        let inspection = table.inspections().next().ok_or("renderer-possession")?;
        if inspection.completed_operations != 0 {
            return Err("renderer-invocation-spent");
        }
        let (node, call) = projection.into_numeric();
        Ok(PreparedRendererInvocation {
            execution: PreparedRendererExecution::checked(
                &self.identity,
                self.bounds,
                &inspection.scope,
                selected.active,
                node.node,
            ),
            node,
            call,
            configuration: configuration.entries.clone(),
            fore: selected.fore.to_vec(),
        })
    }
}
