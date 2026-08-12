use crate::{PartPresentationState, PartsAction, PartsView};
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, CandidateInventory, CandidateObservation,
    DiscoveryProofId, MembershipProofId, PartId,
};
use conduit_core::{bind_active_play, BootId, HostId, LinkBindingId, OfferGeneration, SignId};
use conduit_std_host::{StdHost, StdHostConfig};

fn admit(
    membership: &mut BodyMembership,
    body: &Body,
    subject: &str,
    host: Option<(&str, &str, u64)>,
    index: u64,
) -> PartId {
    let part = PartId::bind(&body.body_id, subject, index).unwrap();
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            MembershipProofId::bind(&format!("proof/{subject}")).unwrap(),
            SignId::from(format!("sign/{subject}/admitted")),
        )
        .unwrap();
    if let Some((host_id, boot_id, generation)) = host {
        membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: HostId::from(host_id),
                    boot_id: BootId::from(boot_id),
                    offer_generation: OfferGeneration(generation),
                    proof_id: MembershipProofId::bind(&format!("proof/{subject}/current")).unwrap(),
                    sequence: 1,
                },
                SignId::from(format!("sign/{subject}/present")),
            )
            .unwrap();
    }
    part
}

#[test]
fn parts_view_derives_here_attached_offline_candidates_and_plan_truth() {
    let std = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("patchbay/std"),
        boot_id: BootId::from("patchbay/std-boot"),
        offer_generation: OfferGeneration(1),
    });
    let expanded = crate::FormEditor::from_source(
        "parts.conduit".into(),
        include_str!("../../../examples/hello.conduit").into(),
    )
    .unwrap()
    .expand_form("hello")
    .unwrap();
    let plan = std.plan_expanded_local(&expanded).unwrap();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_form_id.clone(),
        1,
        SignId::from("sign/parts-body-born"),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let here = admit(
        &mut membership,
        &body,
        "local",
        Some(("patchbay/std", "patchbay/std-boot", 1)),
        0,
    );
    admit(
        &mut membership,
        &body,
        "browser",
        Some(("browser/tab-2", "browser-boot/tab-2", 1)),
        1,
    );
    admit(&mut membership, &body, "pico", None, 2);

    let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let mut candidate_advertisement = std.advertisement().clone();
    candidate_advertisement.host_id = HostId::from("browser/tab-3");
    candidate_advertisement.boot_id = BootId::from("browser-boot/tab-3");
    candidates
        .observe(CandidateObservation {
            advertisement: candidate_advertisement,
            friendly_label: "Browser · tab 3".into(),
            observed_binding_id: LinkBindingId::from("line/browser-tab-3"),
            observation_sign_id: SignId::from("sign/browser-tab-3-observed"),
            proof_id: DiscoveryProofId::bind("proof/browser-tab-3-discovery").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 512,
        })
        .unwrap();
    let play = bind_active_play(
        &plan.plan_id,
        &HostId::from("patchbay/std"),
        &BootId::from("patchbay/std-boot"),
        0,
    );
    let retained_membership = membership.clone();
    let retained_candidates = candidates.clone();
    let view = PartsView::project(
        &body,
        &membership,
        &candidates,
        &here,
        Some(&plan),
        Some(&play),
        true,
    )
    .unwrap();

    assert_eq!(view.parts.len(), 3);
    assert_eq!(view.parts[0].state, PartPresentationState::Here);
    assert!(view.parts[0].in_plan);
    assert!(view.parts[0].playing);
    assert!(!view.parts[0].details.planned_placements.is_empty());
    assert!(view.parts[0].details.expected_signs > 0);
    assert_eq!(view.parts[1].state, PartPresentationState::Attached);
    assert!(!view.parts[1].in_plan);
    assert_eq!(view.parts[2].state, PartPresentationState::Offline);
    assert!(!view.parts[2].available);
    assert!(view.parts[2].details.proof_reference.is_some());
    assert_eq!(view.wants_to_join.len(), 1);
    assert_eq!(
        view.wants_to_join[0].actions,
        vec![
            PartsAction::Inspect,
            PartsAction::Admit,
            PartsAction::Refuse
        ]
    );
    assert!(view.actions.contains(&PartsAction::SpawnBrowserPart));
    assert!(view.actions.contains(&PartsAction::Replan));
    assert!(view.new_realization_possibilities);
    assert_eq!(membership, retained_membership);
    assert_eq!(candidates, retained_candidates);
}

#[test]
fn joining_part_does_not_change_form_or_active_plan_annotations() {
    let std = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("patchbay/std"),
        boot_id: BootId::from("patchbay/std-boot"),
        offer_generation: OfferGeneration(1),
    });
    let expanded = crate::FormEditor::from_source(
        "parts-stable.conduit".into(),
        include_str!("../../../examples/hello.conduit").into(),
    )
    .unwrap()
    .expand_form("hello")
    .unwrap();
    let plan = std.plan_expanded_local(&expanded).unwrap();
    let original_form = plan.checked_form_id.clone();
    let original_plan = plan.clone();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_form_id.clone(),
        2,
        SignId::from("sign/stable-body-born"),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let here = admit(
        &mut membership,
        &body,
        "local-stable",
        Some(("patchbay/std", "patchbay/std-boot", 1)),
        0,
    );
    let candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let before = PartsView::project(
        &body,
        &membership,
        &candidates,
        &here,
        Some(&plan),
        None,
        true,
    )
    .unwrap();
    admit(
        &mut membership,
        &body,
        "new-browser",
        Some(("browser/new", "browser-boot/new", 1)),
        1,
    );
    let after = PartsView::project(
        &body,
        &membership,
        &candidates,
        &here,
        Some(&plan),
        None,
        true,
    )
    .unwrap();
    assert_eq!(before.parts[0].in_plan, after.parts[0].in_plan);
    assert!(!after.parts[1].in_plan);
    assert!(after.new_realization_possibilities);
    assert_eq!(plan, original_plan);
    assert_eq!(plan.checked_form_id, original_form);
}
