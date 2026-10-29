//! Route preparation from locally retained, non-deserializable Body evidence.
use super::*;
use conduit_body::{BodyPlanEvidence, BodyState, SealedBodyLifecycleSession};
use conduit_core::VerifiedPlanMetadata;

impl AdmittedMaskPlotRoutes {
    /// Admit routes for one exact ordinary Mask in the current sealed proposal.
    ///
    /// This retains the structural checks of `new` without reconstructing a
    /// rich Plan. The session, Core verification and privately captured Body
    /// seal supply the evidence; received identifiers cannot recreate it.
    /// This prepares routes only. Face basis, wardrobe policy, current provider
    /// possession and execution admission remain separate checks.
    pub fn from_sealed_session(
        session: &SealedBodyLifecycleSession,
        mask: &crate::MaskPlot,
        verified: &VerifiedPlanMetadata,
        routes: Vec<SealedMaskPlotRoute>,
    ) -> Result<Self, MaskRouteAdmissionError> {
        use MaskRouteAdmissionError as E;
        let evidence = session.evidence();
        let current = session.realization().ok_or(E::InvalidPlan)?;
        let seal = &current.plan;
        if seal.validate_for(&current.wake).is_err()
            || seal.body_id() != &evidence.body.body_id
            || current.wake.workset != evidence.body.workset
            || current.wake.workload_revision != evidence.body.workload_revision
            || !matches!(&evidence.body.state, BodyState::Awake { wake_id } if wake_id == seal.wake_id())
            || current
                .play
                .as_ref()
                .is_some_and(|play| !play.validate_for(seal))
        {
            return Err(E::InvalidPlan);
        }
        crate::mask_plot::validate_verified_mask(mask, verified).map_err(|_| E::InvalidPlan)?;
        let plan = verified.preparation_metadata();
        let chain = seal
            .masks()
            .iter()
            .flat_map(|topology| topology.chains())
            .find(|chain| {
                chain.plan().identity() == &mask.plot_identity
                    && chain.plan().plan_id() == verified.plan_id()
                    && chain.plan().fragments().len() == plan.fragments.len()
                    && chain.plan().fragments().iter().zip(&plan.fragments).all(
                        |(sealed, fragment)| {
                            sealed.fragment_id() == &fragment.fragment_id
                                && sealed.host_id() == &fragment.host_id
                                && sealed.boot_id() == &fragment.boot_id
                                && sealed.offer_generation() == fragment.offer_generation
                        },
                    )
            })
            .ok_or(E::UnsealedMaskPlot)?;
        for fragment in &plan.fragments {
            if !evidence.membership.parts.iter().any(|part| {
                part.is_present()
                    && part.current.as_ref().is_some_and(|host| {
                        host.host_id == fragment.host_id && host.boot_id == fragment.boot_id
                    })
            }) {
                return Err(E::InvalidPlan);
            }
        }
        let placements = || {
            plan.fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
        };
        for route in &routes {
            if &route.plan_id != seal.plan_id() {
                return Err(E::WrongPlan);
            }
            if route.mask_plot != mask.plot_identity {
                return Err(E::UnknownMaskPlot);
            }
            if route.placement_ids.len() != placements().count()
                || route.placement_ids.iter().enumerate().any(|(index, id)| {
                    route.placement_ids[..index].contains(id)
                        || !placements().any(|placement| &placement.placement_id == id)
                })
                || chain
                    .stages()
                    .iter()
                    .any(|id| !route.placement_ids.contains(id))
            {
                return Err(E::MissingPlacement);
            }
            for boundary in [
                &mask.face_input,
                &mask.interaction_output,
                &mask.show_output,
            ] {
                if !placements().any(|placement| {
                    placement.gear_id == boundary.gear_id
                        && route.placement_ids.contains(&placement.placement_id)
                }) {
                    return Err(E::MissingBoundary);
                }
            }
        }
        Ok(Self {
            body_id: seal.body_id().clone(),
            plan_id: seal.plan_id().clone(),
            routes,
        })
    }
}
