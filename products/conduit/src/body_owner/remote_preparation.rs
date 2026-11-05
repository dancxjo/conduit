//! Preparation from retained owner truth, never caller-supplied approval bytes.
use super::Owner;
use conduit_body::{BodyPlayIdentity, PreparedBodyFragment};
use conduit_core::HostAdvertisement;
use conduit_embedded_build::{
    generate_prepared_body_workset, BodyFragmentGenerationRefusal, BodyFragmentProjectionLimits,
    BoundResidentExpression, GeneratedBodyWorksetDocument,
};

impl Owner {
    /// Resolve the selected resident implementations from current member offers;
    /// no arbitrary encoded projection crosses this owner preparation boundary.
    #[allow(dead_code)]
    pub(super) fn prepare_remote_resident_workset(
        &self,
        actual: &HostAdvertisement,
        images: &[&conduit_embedded_build::PreparedResidentExpression],
        limits: BodyFragmentProjectionLimits,
    ) -> Result<(BodyPlayIdentity, GeneratedBodyWorksetDocument), String> {
        self.prepare_remote_body_workset(actual, limits.maximum_document_bytes, |scope| {
            let resident = conduit_embedded_build::bind_current_resident_expressions(
                scope.plot_plan(),
                &scope.fragment().fragment_id,
                actual,
                images,
            )
            .map_err(BodyFragmentGenerationRefusal::Resident)?;
            Ok((resident, limits))
        })
    }
    // The installed remote dispatcher must call this before signing or sending.
    // It is not wired to Start while native receiver installation is absent.
    #[allow(dead_code)]
    pub(super) fn prepare_remote_body_workset(
        &self,
        actual: &HostAdvertisement,
        maximum_document_bytes: u32,
        resolve: impl FnMut(
            PreparedBodyFragment<'_>,
        ) -> Result<
            (Vec<BoundResidentExpression>, BodyFragmentProjectionLimits),
            BodyFragmentGenerationRefusal,
        >,
    ) -> Result<(BodyPlayIdentity, GeneratedBodyWorksetDocument), String> {
        if self.host.is_playing() || actual.host_id == self.host.advertisement().host_id {
            return Err("remote preparation requires the idle original owner Host".into());
        }
        self.require_observed_member_offer(actual)?;
        let proposal = self
            .session
            .realization()
            .ok_or("checked Body proposal required")?;
        if proposal.play.is_some() {
            return Err("Body proposal is already Playing".into());
        }
        let play = self
            .host
            .prospective_body_play(&proposal.wake, &proposal.plan)?;
        let fragment = proposal
            .plan
            .plots
            .iter()
            .map(|plot| &plot.plan)
            .chain(
                proposal
                    .plan
                    .mask_topologies
                    .iter()
                    .flat_map(|topology| topology.chains.iter().map(|chain| &chain.plan)),
            )
            .flat_map(|plan| &plan.fragments)
            .find(|fragment| fragment.host_id == actual.host_id)
            .ok_or("current Body proposal has no fragment for that member")?;
        let scope = PreparedBodyFragment::prepare(&proposal.wake, &proposal.plan, &play, fragment)
            .map_err(|_| "current Body fragment containment refused")?;
        let document =
            generate_prepared_body_workset(scope, actual, maximum_document_bytes, resolve)
                .map_err(|error| format!("remote Body projection refused: {error:?}"))?;
        Ok((play, document))
    }
}
