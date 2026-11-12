//! Preparation from retained owner truth, never caller-supplied approval bytes.
use super::Owner;
use conduit_body::ResidentPlot;
use conduit_body::{BodyPlayIdentity, PreparedBodyFragment};
use conduit_core::HostAdvertisement;
use conduit_embedded_build::{
    generate_prepared_body_workset, generate_prepared_body_workset_with_line_fore,
    BodyFragmentGenerationRefusal, BodyFragmentProjectionLimits, BoundResidentExpression,
    GeneratedBodyWorksetDocument, SelectedForeSession,
};
use std::collections::BTreeMap;

impl Owner {
    /// The installed File policy already prepared these immutable codebooks.
    /// Neither the transport nor its challenge supplies Source or projections.
    #[cfg(target_os = "linux")]
    pub(crate) fn approve_selected_remote_workset(
        &mut self,
        member: &crate::durable_host::owner::MemberPresentationBasis,
        challenge: &conduit_body::HostCommandChallenge<'_>,
        limits: BodyFragmentProjectionLimits,
    ) -> Result<(Vec<u8>, [u8; 64]), String> {
        self.approve_selected_remote_workset_with_line_fore(
            member,
            challenge,
            limits,
            &BTreeMap::new(),
        )
    }
    /// Sessions are supplied by original native policy, never the challenge.
    #[cfg(target_os = "linux")]
    pub(crate) fn approve_selected_remote_workset_with_line_fore(
        &mut self,
        member: &crate::durable_host::owner::MemberPresentationBasis,
        challenge: &conduit_body::HostCommandChallenge<'_>,
        limits: BodyFragmentProjectionLimits,
        sessions: &BTreeMap<ResidentPlot, Vec<SelectedForeSession>>,
    ) -> Result<(Vec<u8>, [u8; 64]), String> {
        let retained = self
            .selected_file_workset
            .take()
            .ok_or("selected checked File custody absent")?;
        let result = (|| {
            retained.verify(&self.host, &self.session.evidence().body.workset)?;
            let images: Vec<_> = retained.checked.resident_images.iter().collect();
            let mut native = Vec::new();
            for offer in &member.host_advertisement.capabilities {
                if images
                    .iter()
                    .any(|image| image.offer_description() == offer)
                {
                    continue;
                }
                native.push(offer);
            }
            let command = self.approve_remote_workset_with_line_fore(
                member, challenge, &images, &native, limits, sessions,
            )?;
            Ok((command.payload().to_vec(), *command.signature()))
        })();
        self.selected_file_workset = Some(retained);
        result
    }
    /// Resolve the selected resident implementations from current member offers;
    /// no arbitrary encoded projection crosses this owner preparation boundary.
    #[allow(dead_code)]
    pub(super) fn prepare_remote_resident_workset(
        &self,
        actual: &HostAdvertisement,
        images: &[&conduit_embedded_build::PreparedResidentExpression],
        limits: BodyFragmentProjectionLimits,
    ) -> Result<(BodyPlayIdentity, GeneratedBodyWorksetDocument), String> {
        self.prepare_remote_mixed_workset(actual, images, &[], limits)
    }

    /// Native offers are explicitly selected descriptive inputs, not possession
    /// or grants. The receiver must separately authorize and admit the exact
    /// native operation before execution. Resident-only selection stays strict.
    pub(super) fn prepare_remote_mixed_workset(
        &self,
        actual: &HostAdvertisement,
        images: &[&conduit_embedded_build::PreparedResidentExpression],
        native: &[&conduit_core::CapabilityOffer],
        limits: BodyFragmentProjectionLimits,
    ) -> Result<(BodyPlayIdentity, GeneratedBodyWorksetDocument), String> {
        self.prepare_remote_mixed_workset_with_line_fore(
            actual,
            images,
            native,
            limits,
            &BTreeMap::new(),
        )
    }
    pub(super) fn prepare_remote_mixed_workset_with_line_fore(
        &self,
        actual: &HostAdvertisement,
        images: &[&conduit_embedded_build::PreparedResidentExpression],
        native: &[&conduit_core::CapabilityOffer],
        limits: BodyFragmentProjectionLimits,
        sessions: &BTreeMap<ResidentPlot, Vec<SelectedForeSession>>,
    ) -> Result<(BodyPlayIdentity, GeneratedBodyWorksetDocument), String> {
        self.project_remote_body_workset(actual, |anchor| {
            let mut local_plots = std::collections::BTreeSet::new();
            for local in anchor.local_fragments() {
                let local = local.map_err(|_| BodyFragmentGenerationRefusal::Scope)?;
                let fragment = local.fragment();
                local_plots.insert(ResidentPlot::new(
                    fragment.source_document_id.clone(),
                    fragment.checked_plot_id.clone(),
                ));
            }
            if sessions
                .iter()
                .any(|(plot, rows)| !local_plots.contains(plot) || rows.len() > 16)
            {
                return Err(BodyFragmentGenerationRefusal::Scope);
            }
            generate_prepared_body_workset_with_line_fore(
                anchor,
                actual,
                limits.maximum_document_bytes,
                |scope| {
                    let local_native: Vec<_> = native
                        .iter()
                        .copied()
                        .filter(|offer| {
                            scope
                                .fragment()
                                .placements
                                .iter()
                                .any(|placement| placement.capability_id == offer.capability_id)
                        })
                        .collect();
                    if local_native.iter().any(|offer| {
                        offer.implementation.execution_profile_id.as_str()
                            == conduit_embedded_build::RESIDENT_EXPRESSION_PROFILE
                    }) {
                        return Err(BodyFragmentGenerationRefusal::Resident(
                            conduit_embedded_build::ResidentExpressionRefusal::Selection,
                        ));
                    }
                    let resident = conduit_embedded_build::bind_current_mixed_resident_expressions(
                        scope.plot_plan(),
                        &scope.fragment().fragment_id,
                        actual,
                        images,
                        &local_native,
                    )
                    .map_err(BodyFragmentGenerationRefusal::Resident)?;
                    let fragment = scope.fragment();
                    let plot = ResidentPlot::new(
                        fragment.source_document_id.clone(),
                        fragment.checked_plot_id.clone(),
                    );
                    let selected = sessions
                        .get(&plot)
                        .map(|rows| {
                            rows.iter()
                                .map(|row| SelectedForeSession {
                                    front_port_id: row.front_port_id.clone(),
                                    direction: row.direction,
                                    track: row.track,
                                    session: row.session,
                                    session_contract: row.session_contract,
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    Ok((resident, limits, selected))
                },
            )
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
        self.project_remote_body_workset(actual, |scope| {
            generate_prepared_body_workset(scope, actual, maximum_document_bytes, resolve)
        })
    }

    /// Resolve exact sessions for the original sealed Fore routes together with
    /// their resident implementations. Session metadata is descriptive: this
    /// entrance neither installs a carrier nor signs or admits the document.
    #[allow(dead_code)] // Installed carrier integration supplies this preparation resolver.
    pub(super) fn prepare_remote_body_workset_with_line_fore(
        &self,
        actual: &HostAdvertisement,
        maximum_document_bytes: u32,
        resolve: impl FnMut(
            PreparedBodyFragment<'_>,
        ) -> Result<
            (
                Vec<BoundResidentExpression>,
                BodyFragmentProjectionLimits,
                Vec<SelectedForeSession>,
            ),
            BodyFragmentGenerationRefusal,
        >,
    ) -> Result<(BodyPlayIdentity, GeneratedBodyWorksetDocument), String> {
        self.project_remote_body_workset(actual, |scope| {
            generate_prepared_body_workset_with_line_fore(
                scope,
                actual,
                maximum_document_bytes,
                resolve,
            )
        })
    }

    // Both preparation entrances derive scope solely from original owner truth.
    fn project_remote_body_workset(
        &self,
        actual: &HostAdvertisement,
        project: impl FnOnce(
            PreparedBodyFragment<'_>,
        )
            -> Result<GeneratedBodyWorksetDocument, BodyFragmentGenerationRefusal>,
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
            project(scope).map_err(|error| format!("remote Body projection refused: {error:?}"))?;
        Ok((play, document))
    }
}
