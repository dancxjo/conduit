//! Correlate the admitted Home Face route with the scheduler's actual Back.
use super::*;
use crate::body_execution::face_contribution::FaceContributionContext;
use crate::installed_std::{ambient_home_factory, ambient_home_route::AmbientHomeRoute};
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_std_offers::AmbientFaceProjection;

impl BodyKernel<'_> {
    pub(crate) fn prepare_home_face_routes(
        &mut self,
        wake: &Wake,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
        host: &conduit_core::HostAdvertisement,
        has_observer: bool,
    ) -> Result<(), String> {
        let projects = self
            .operations
            .iter()
            .filter(|operation| operation.call.0 == 0 && is_home_face_call(operation));
        if !has_observer && projects.clone().next().is_some() {
            return Err("Home Face requires an installed contribution observer".into());
        }
        let mut routes = Vec::with_capacity(projects.clone().count());
        for operation in projects {
            let (partition, placement_id) = self
                .partitions
                .iter()
                .find_map(|partition| {
                    partition
                        .placement_for_node(operation.node)
                        .map(|placement| (partition, placement))
                })
                .ok_or("Home Face project call has no lowered placement")?;
            let placement = plan
                .plots
                .iter()
                .filter(|plot| plot.plan.plan_id == partition.plan_id)
                .flat_map(|plot| &plot.plan.fragments)
                .filter(|fragment| fragment.fragment_id == partition.fragment_id)
                .flat_map(|fragment| &fragment.placements)
                .find(|placement| &placement.placement_id == placement_id)
                .ok_or("Home Face placement is absent from the current Body Plan")?;
            let Some(InstalledBack::AmbientHome(back)) =
                self.scheduler.drivers().get(usize::from(operation.node.0))
            else {
                return Err("Home Face project call has no installed Home Back".into());
            };
            routes.push(AmbientHomeRoute::prepare(
                FaceContributionContext {
                    wake,
                    plan,
                    play,
                    host,
                    partition,
                    node: operation.node,
                    call: operation.call,
                    contract: &operation.contract_id,
                },
                back.observation(),
                ambient_home_factory::label(placement)?,
                ambient_home_factory::projection(placement)?,
            )?);
        }
        self.home_face_routes = routes;
        Ok(())
    }
}

pub(super) fn is_home_face_call(operation: &LoweredHostCall) -> bool {
    [AmbientFaceProjection::Home, AmbientFaceProjection::Clock]
        .into_iter()
        .any(|projection| {
            let (project, custody) = projection.calls();
            (match operation.call.0 {
                0 => operation.contract_id.as_str() == project,
                1 => operation.contract_id.as_str() == custody,
                _ => false,
            }) && operation
                .target_kind
                .as_ref()
                .is_some_and(|kind| kind.as_str() == projection.kind())
                && operation.maximum_in_flight == 1
                && operation.binding.maximum_input_bytes == projection.input_bytes()
                && operation.binding.maximum_output_bytes == 0
        })
}
