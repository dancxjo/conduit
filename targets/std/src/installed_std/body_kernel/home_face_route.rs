//! Home dispatch uses the same installed kernel, original IMAGE and observer.
use super::*;
use crate::body_execution::{
    face_contribution::FaceContributionContext, BodyFaceContributionObserver,
};
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_core::HostAdvertisement;

impl BodyKernel<'_> {
    pub(crate) fn attach_home_face(
        &mut self,
        wake: &Wake,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
        host: &HostAdvertisement,
        observer: bool,
        original_image: bool,
    ) -> Result<(), String> {
        let mut count = 0;
        let mut aspects = [None; 2];
        for (index, driver) in self.scheduler.drivers().iter().enumerate() {
            let InstalledBack::AmbientHome(back) = driver else {
                continue;
            };
            if !observer || !original_image || count >= 2 || self.home_face[count].is_some() {
                return Err("Home requires one observer and original IMAGE ownership".into());
            }
            let node =
                conduit_kernel::NodeId(u16::try_from(index).map_err(|_| "Home node overflow")?);
            let partition = self
                .partitions
                .iter()
                .find(|p| p.placement_for_node(node).is_some())
                .ok_or("Home lacks its actual partition")?;
            let placement_id = partition
                .placement_for_node(node)
                .ok_or("Home placement missing")?;
            let placement = plan
                .plots
                .iter()
                .filter(|plot| plot.plan.plan_id == partition.plan_id)
                .flat_map(|plot| &plot.plan.fragments)
                .flat_map(|fragment| &fragment.placements)
                .find(|placement| &placement.placement_id == placement_id)
                .ok_or("Home original placement missing")?;
            let aspect = conduit_ambient_display::AmbientDisplayAspect::from_kind(
                placement.kind_id.as_str(),
            )
            .ok_or("unknown ambient contribution")?;
            if aspects.iter().flatten().any(|previous| {
                *previous == aspect
                    || *previous == conduit_ambient_display::AmbientDisplayAspect::Home
                    || aspect == conduit_ambient_display::AmbientDisplayAspect::Home
            }) {
                return Err(
                    "ambient facets must be distinct; full Home has one contribution".into(),
                );
            }
            aspects[count] = Some(aspect);
            let policy = super::super::ambient_home_factory::configuration_policy(placement)?;
            let resident = policy == conduit_ambient_display::AmbientUpdatePolicy::Resident;
            let (_, _, project_contract, custody_contract) = if resident {
                conduit_std_offers::ambient_resident_aspect_identity(aspect)
            } else {
                conduit_std_offers::ambient_aspect_identity(aspect)
            };
            let project = self
                .operations
                .iter()
                .find(|op| op.node == node && op.contract_id.as_str() == project_contract)
                .ok_or("Home lacks exact project operation")?;
            let custody = self
                .operations
                .iter()
                .find(|op| op.node == node && op.contract_id.as_str() == custody_contract)
                .ok_or("Home lacks exact custody operation")?;
            if project.call != conduit_kernel::HostCallId(0)
                || custody.call != conduit_kernel::HostCallId(1)
                || project.binding.maximum_input_bytes
                    != conduit_semantic_catalog::AMBIENT_HOME_MAXIMUM_BYTES
                || custody.binding.maximum_input_bytes != project.binding.maximum_input_bytes
                || project.binding.maximum_output_bytes != 0
                || custody.binding.maximum_output_bytes != 0
                || project.target_kind != custody.target_kind
                || project.target_kind.as_ref().map(|k| k.as_str())
                    != Some(placement.kind_id.as_str())
            {
                return Err("Home planned operation shape differs".into());
            }
            let label = conduit_core::face_data_label(&placement.configuration)?;
            self.home_face[count] = Some(
                super::super::ambient_home_route::AmbientHomeRoute::prepare_profile(
                    FaceContributionContext {
                        wake,
                        plan,
                        play,
                        host,
                        partition,
                        node,
                        call: project.call,
                        contract: &project.contract_id,
                    },
                    back.observation(),
                    label,
                    policy,
                    aspect,
                )?,
            );
            count += 1;
        }
        Ok(())
    }
    pub(super) fn home_face_call(&mut self, request: HostCallRequest) -> Result<bool, String> {
        let Some(route) = self
            .home_face
            .iter_mut()
            .flatten()
            .find(|r| r.handles(request))
        else {
            return Ok(false);
        };
        route.dispatch(self.scheduler.as_mut(), request)?;
        Ok(true)
    }
    pub(super) fn poll_home_face(
        &mut self,
        observer: &mut Option<&mut dyn BodyFaceContributionObserver>,
    ) -> Result<(), String> {
        for slot in &mut self.home_face {
            let Some(route) = slot.as_mut() else {
                continue;
            };
            if route.is_retired() {
                *slot = None;
                continue;
            }
            let observer = observer.as_deref_mut().ok_or("Home observer detached")?;
            route.poll(self.scheduler.as_mut(), observer)?;
        }
        Ok(())
    }
    pub(super) fn semantic_face_pending(&self) -> bool {
        self.chart_face_pending()
            || self
                .home_face
                .iter()
                .flatten()
                .any(|route| route.is_pending())
    }
}
