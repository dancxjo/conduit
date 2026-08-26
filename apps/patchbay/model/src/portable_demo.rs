//! One living, bounded semantic input shared by renderer adapter proofs.

use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, CandidateInventory, CandidateObservation,
    DiscoveryProofId, HostPresenceClock, HostPresenceClockScale, HostPresenceTable,
    MembershipProofId, PartId,
};
use conduit_core::{bind_active_play, BootId, HostId, LinkBindingId, OfferGeneration, SignId};
use conduit_presentation::{Presentation, TemporalInstant, TemporalReference, TemporalScale};

use crate::{
    DistributedRouteDemo, FormEditor, PartsView, PatchbayHostAdapter, PatchbayHostProfile,
    PatchbayModel, PatchbayPresentation, PatchbayRequestId, PatchbayTopology, PlanDocument,
    PlayDocument,
};

pub fn portable_demonstration_with_adapter(
    adapter: &dyn PatchbayHostAdapter,
) -> Result<Presentation, String> {
    portable_demonstration_with_parts_and_adapter(adapter).map(|(presentation, _)| presentation)
}

pub fn portable_demonstration_with_parts_and_adapter(
    adapter: &dyn PatchbayHostAdapter,
) -> Result<(Presentation, PartsView), String> {
    let editor = FormEditor::from_source(
        "examples/hello.conduit".into(),
        include_str!("../../../../examples/hello.conduit").into(),
    )
    .map_err(|error| error.to_string())?;
    let expanded = editor
        .expand_form("hello")
        .map_err(|error| error.to_string())?;
    let advertisement = adapter.advertisement(
        HostId::from("patchbay-portable/host"),
        BootId::from("patchbay-portable/boot"),
        OfferGeneration(1),
        PatchbayHostProfile::Reference,
    )?;
    let host_id = advertisement.host_id.clone();
    let boot_id = advertisement.boot_id.clone();
    let plan = adapter.plan_expanded_local(&advertisement, &expanded)?;
    let plan_document = PlanDocument::from_plan(
        PatchbayRequestId::new("patchbay/portable-plan").map_err(|error| format!("{error:?}"))?,
        &plan,
    )
    .map_err(|error| format!("{error:?}"))?;
    let execution = adapter.run_fragment(&advertisement, plan.fragments[0].clone())?;
    let play_document = PlayDocument::from_execution(&plan, &execution.projection)
        .map_err(|error| format!("{error:?}"))?;
    let patchbay = PatchbayModel::from_advertisement(advertisement.clone());
    let mut topology = PatchbayTopology::new(1).map_err(|error| error.to_string())?;
    topology
        .ingest(&patchbay.startup_snapshot())
        .map_err(|error| error.to_string())?;
    let route = DistributedRouteDemo::build_for_source(advertisement.clone())
        .map_err(|error| format!("{error:?}"))?
        .presentation()
        .clone();
    let projection = PatchbayPresentation::new(
        1,
        editor.view(),
        Some(plan_document),
        Some(play_document),
        topology.current_report().cloned(),
        vec![route],
    )
    .map_err(|error| error.to_string())?
    .with_graph(crate::PatchbayGraph::from_expanded(&expanded).map_err(|error| error.to_string())?)
    .map_err(|error| error.to_string())?;
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_form_id.clone(),
        1,
        SignId::from("patchbay/bornd"),
    )
    .map_err(|error| error.to_string())?;
    let (body, wake) = body
        .wake(1, SignId::from("patchbay/woke"))
        .map_err(|error| error.to_string())?;
    let wake = wake
        .plan_ready(&plan, SignId::from("patchbay/planned"))
        .map_err(|error| error.to_string())?;
    let active_play = bind_active_play(&plan.plan_id, &host_id, &boot_id, 0);
    let wake = wake
        .play_started(&active_play, SignId::from("patchbay/playing"))
        .map_err(|error| error.to_string())?;
    let mut membership =
        BodyMembership::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
    let here = admit_demo_part(
        &mut membership,
        &body,
        "here",
        Some((&host_id, &boot_id, OfferGeneration(1))),
        0,
    )?;
    let browser = admit_demo_part(
        &mut membership,
        &body,
        "browser-tab-2",
        Some((
            &HostId::from("browser/tab-2"),
            &BootId::from("browser/tab-2/boot"),
            OfferGeneration(1),
        )),
        1,
    )?;
    admit_demo_part(&mut membership, &body, "pico-w", None, 2)?;
    let mut candidates =
        CandidateInventory::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
    let mut candidate = advertisement;
    candidate.host_id = HostId::from("browser/tab-3");
    candidate.boot_id = BootId::from("browser/tab-3/boot");
    candidates
        .observe(CandidateObservation {
            advertisement: candidate,
            friendly_label: "Browser · tab 3".into(),
            observed_binding_id: LinkBindingId::from("patchbay/browser-tab-3/observed"),
            observation_sign_id: SignId::from("patchbay/browser-tab-3/observed"),
            proof_id: DiscoveryProofId::bind("patchbay/browser-tab-3/discovery")
                .map_err(|error| format!("{error:?}"))?,
            freshness_sequence: 1,
            encoded_bytes: 512,
        })
        .map_err(|error| format!("{error:?}"))?;
    let presence_clock = HostPresenceClock::new(
        "clock/patchbay-documentary".into(),
        HostPresenceClockScale::Milliseconds,
        1,
        1,
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut presence = HostPresenceTable::new(body.body_id.clone(), presence_clock, 30_000)
        .map_err(|error| format!("{error:?}"))?;
    let browser_session = LinkBindingId::from("patchbay/browser-tab-2/presence");
    presence
        .start(
            &membership,
            &browser,
            browser_session.clone(),
            1,
            1_000,
            20_000,
            SignId::from("patchbay/browser-tab-2/presence-started"),
        )
        .map_err(|error| format!("{error:?}"))?;
    presence
        .renew(
            &membership,
            &browser,
            &browser_session,
            2,
            12_000,
            30_000,
            SignId::from("patchbay/browser-tab-2/presence-renewed"),
        )
        .map_err(|error| format!("{error:?}"))?;
    let parts = PartsView::project_with_presence(
        &body,
        &membership,
        &candidates,
        &here,
        Some(&plan),
        Some(&active_play),
        true,
        Some(&presence),
    )
    .map_err(|error| format!("{error:?}"))?;
    let presentation = projection
        .to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &parts,
            TemporalReference {
                identity: "reference/patchbay-documentary".into(),
                instant: TemporalInstant {
                    ticks: 17_000,
                    scale: TemporalScale::Milliseconds,
                    clock_basis: "clock/patchbay-documentary".into(),
                    resolution_ticks: 1,
                    uncertainty_ticks: 0,
                },
            },
        )
        .map_err(|error| error.to_string())?;
    Ok((presentation, parts))
}

#[cfg(test)]
pub fn portable_demonstration() -> Result<Presentation, String> {
    portable_demonstration_with_adapter(crate::host_adapter::test_host_adapter())
}

#[cfg(test)]
pub fn portable_demonstration_with_parts() -> Result<(Presentation, PartsView), String> {
    portable_demonstration_with_parts_and_adapter(crate::host_adapter::test_host_adapter())
}

fn admit_demo_part(
    membership: &mut BodyMembership,
    body: &Body,
    subject: &str,
    current: Option<(&HostId, &BootId, OfferGeneration)>,
    index: u64,
) -> Result<PartId, String> {
    let part = PartId::bind(&body.body_id, subject, index).map_err(|error| format!("{error:?}"))?;
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            MembershipProofId::bind(&format!("patchbay/{subject}/admitted"))
                .map_err(|error| format!("{error:?}"))?,
            SignId::from(format!("patchbay/{subject}/admitted")),
        )
        .map_err(|error| format!("{error:?}"))?;
    if let Some((host_id, boot_id, offer_generation)) = current {
        membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: host_id.clone(),
                    boot_id: boot_id.clone(),
                    offer_generation,
                    proof_id: MembershipProofId::bind(&format!("patchbay/{subject}/current"))
                        .map_err(|error| format!("{error:?}"))?,
                    sequence: 1,
                },
                SignId::from(format!("patchbay/{subject}/present")),
            )
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(part)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documentary_fixture_keeps_exact_semantic_identities() {
        let first = portable_demonstration().unwrap();
        let second = portable_demonstration().unwrap();
        assert_eq!(first.identity, second.identity);
        assert_eq!(first.basis.plan_id, second.basis.plan_id);
        assert_eq!(first.basis.active_play_id, second.basis.active_play_id);
        assert_eq!(first.subjects, second.subjects);
        assert!(first.subjects.iter().any(|subject| {
            subject.role == conduit_presentation::PresentationRole::Body
                && subject.identity
                    == format!("body/{}", first.basis.body_id.as_ref().unwrap().as_str())
        }));
        assert!(first.subjects.iter().any(|subject| {
            subject.role == conduit_presentation::PresentationRole::Part
                && subject.identity.starts_with("part/")
        }));
        assert!(first.subjects.iter().any(|subject| {
            subject.role == conduit_presentation::PresentationRole::Host
                && subject.identity.starts_with("host/")
                && subject.identity.contains("/boot/")
        }));
    }

    #[test]
    fn ordinary_startup_has_no_synthetic_diagnostics() {
        let presentation = portable_demonstration().unwrap();

        assert!(presentation
            .subjects
            .iter()
            .all(|subject| subject.role != conduit_presentation::PresentationRole::Diagnostic));
    }
}
