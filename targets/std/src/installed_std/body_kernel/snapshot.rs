//! Exact finite snapshot custody for the ordinary Body kernel. No scheduler,
//! grant issuer, retry, State fallback or acknowledgement is introduced here.
use super::*;
use crate::body_execution::BodySnapshotSelection;
use conduit_body::{BodyPlayIdentity, BodyPlotPlan};
use conduit_linux_snapshot_store::PreparedFileSnapshotOperation;

pub(super) struct PreparedBodySnapshot<'a> {
    play: BodyPlayIdentity,
    confirmed: bool,
    node: conduit_kernel::NodeId,
    files: PreparedFileSnapshotOperation<'a>,
}
impl<'a> PreparedBodySnapshot<'a> {
    fn prepare(
        partitions: &[BodyPlotPlan],
        identities: &[KernelIdentityMap],
        selection: BodySnapshotSelection<'a>,
        play: &BodyPlayIdentity,
    ) -> Result<Self, String> {
        if selection.body_id != &play.body_id || selection.plan_id != &play.plan_id {
            return Err("snapshot custody belongs to another Body Plan/Play".into());
        }
        let mut selected = partitions
            .iter()
            .flat_map(|partition| partition.plan.fragments.iter())
            .flat_map(|fragment| {
                fragment
                    .placements
                    .iter()
                    .map(move |placement| (fragment, placement))
            })
            .filter(|(_, placement)| {
                matches!(
                    placement.implementation_id.as_str(),
                    crate::linux_snapshot::PUBLISH | crate::linux_snapshot::READ
                )
            });
        let (fragment, placement) = selected
            .next()
            .ok_or("Body has no selected snapshot operation")?;
        if selected.next().is_some() {
            return Err("one snapshot custody admits exactly one selected operation".into());
        }
        let [resource] = placement.resources.as_slice() else {
            return Err("snapshot needs its exact single admitted resource".into());
        };
        let [authority] = placement.authority.as_slice() else {
            return Err("snapshot needs its exact independent grant".into());
        };
        let grant = selection.authority;
        let offer = selection.resource;
        let content = offer
            .content
            .as_ref()
            .ok_or("snapshot selected resource has no content residence")?;
        if resource.pool_id != offer.pool_id
            || resource.class_id != offer.class_id
            || resource.units == 0
            || resource.units > offer.capacity_units
            || resource.content.as_ref() != Some(content)
            || content.owner_host != placement.host_id
            || content.owner_boot != placement.boot_id
            || placement.host_id != grant.host_id
            || placement.boot_id != grant.boot_id
            || placement.capability_id != grant.capability_id
            || authority.grant_id != grant.grant_id
            || authority.contract_id != grant.contract_id
            || authority.host_call_contract_id != grant.host_call_contract_id
            || authority.subject_kind != grant.subject_kind
            || authority.host_id != grant.host_id
            || authority.boot_id != grant.boot_id
            || authority.capability_id != grant.capability_id
        {
            return Err(
                "snapshot resource/residence/grant differs from exact selected placement".into(),
            );
        }
        let identity = identities
            .iter()
            .find(|identity| {
                identity.plan_id == fragment.plan_id && identity.fragment_id == fragment.fragment_id
            })
            .ok_or("snapshot fragment has no exact lowered identity")?;
        let node = identity
            .placements
            .iter()
            .find(|(_, id)| id == &placement.placement_id)
            .map(|(node, _)| *node)
            .ok_or("snapshot placement has no lowered node")?;
        let files = PreparedFileSnapshotOperation::prepare(selection.directory, placement, node)
            .map_err(|error| format!("prepare selected Body snapshot: {error:?}"))?;
        Ok(Self {
            play: play.clone(),
            confirmed: false,
            node,
            files,
        })
    }
    pub(super) fn perform(
        &mut self,
        scheduler: &mut InstalledScheduler,
        request: HostCallRequest,
    ) -> Result<(), String> {
        if !self.confirmed || self.node != request.node {
            return Err("snapshot request has no confirmed exact Body Play/node custody".into());
        }
        super::super::linux_snapshot_host::perform_snapshot(scheduler, request, &mut self.files)
    }
}
impl<'a> BodyKernel<'a> {
    pub(crate) fn attach_snapshot(
        &mut self,
        partitions: &[BodyPlotPlan],
        selection: BodySnapshotSelection<'a>,
        play: &BodyPlayIdentity,
    ) -> Result<(), String> {
        if self.snapshot.is_some() {
            return Err("snapshot custody was already prepared".into());
        }
        self.snapshot = Some(PreparedBodySnapshot::prepare(
            partitions,
            &self.partitions,
            selection,
            play,
        )?);
        Ok(())
    }
    pub(crate) fn confirm_snapshot_play(&mut self, play: &BodyPlayIdentity) -> Result<(), String> {
        if let Some(snapshot) = &mut self.snapshot {
            if snapshot.confirmed || &snapshot.play != play {
                return Err("snapshot issued Play differs from preparation".into());
            }
            snapshot.confirmed = true;
        }
        Ok(())
    }
}

impl BodyKernel<'_> {
    /// The resource snapshot protocol is also used by the exact prepared Text
    /// file provider. Contract identity alone cannot select its realization.
    pub(super) fn requires_snapshot_custody(&self, operation: &LoweredHostCall) -> bool {
        super::super::linux_snapshot_host::is_operation(operation.contract_id.as_str())
            && !self.native.contains(operation.node)
            && !self
                .data_text_hosts
                .iter()
                .any(|host| host.contains_node(operation.node))
    }
}
