//! One living, bounded semantic input shared by renderer adapter proofs.

use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, CandidateInventory, CandidateObservation,
    DiscoveryProofId, MembershipProofId, PartId,
};
use conduit_core::{bind_active_play, BootId, HostId, LinkBindingId, OfferGeneration, SignId};
use conduit_presentation::Presentation;
use conduit_std_host::{StdHost, StdHostConfig, ThreadTimer};

use crate::{
    DistributedRouteDemo, FormEditor, PartsView, PatchbayModel, PatchbayPresentation,
    PatchbayRequestId, PatchbayTopology, PlanDocument, PlayDocument,
};

pub fn portable_demonstration() -> Result<Presentation, String> {
    portable_demonstration_with_parts().map(|(presentation, _)| presentation)
}

pub fn portable_demonstration_with_parts() -> Result<(Presentation, PartsView), String> {
    let editor = FormEditor::from_source(
        "examples/hello.conduit".into(),
        include_str!("../../../examples/hello.conduit").into(),
    )
    .map_err(|error| error.to_string())?;
    let expanded = editor
        .expand_form("hello")
        .map_err(|error| error.to_string())?;
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("patchbay-portable/host"),
        boot_id: BootId::from("patchbay-portable/boot"),
        offer_generation: OfferGeneration(1),
    });
    let host_id = host.advertisement().host_id.clone();
    let boot_id = host.advertisement().boot_id.clone();
    let plan = host
        .plan_expanded_local(&expanded)
        .map_err(|error| error.to_string())?;
    let plan_document = PlanDocument::from_plan(
        PatchbayRequestId::new("patchbay/portable-plan").map_err(|error| format!("{error:?}"))?,
        &plan,
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut output = Vec::with_capacity(4096);
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut ThreadTimer)
        .map_err(|error| error.to_string())?;
    let play_document =
        PlayDocument::from_report(&plan, &report).map_err(|error| format!("{error:?}"))?;
    let patchbay = PatchbayModel::with_identity("patchbay/host".into(), "patchbay/boot".into());
    let mut topology = PatchbayTopology::new(1).map_err(|error| error.to_string())?;
    topology
        .ingest(&patchbay.startup_snapshot())
        .map_err(|error| error.to_string())?;
    let route = DistributedRouteDemo::build()
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
    admit_demo_part(
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
    let mut candidate = host.advertisement().clone();
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
    let parts = PartsView::project(
        &body,
        &membership,
        &candidates,
        &here,
        Some(&plan),
        Some(&active_play),
        true,
    )
    .map_err(|error| format!("{error:?}"))?;
    let presentation = projection
        .to_portable(&body, &wake)
        .map_err(|error| error.to_string())?;
    Ok((presentation, parts))
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
