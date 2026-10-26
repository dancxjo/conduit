//! Exact installed-proposal provenance for the local producer Face. No Face
//! storage, task, physical provider or Body startup is synthesized by this proof.
use conduit_body::{BodyPlanEvidence, *};
use conduit_core as c;
#[path = "../../../../architecture/core/tests/common/sealed_state.rs"]
mod core_fixture;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_local_face_scope.rs"]
mod local_scope;
fn fixture() -> (SealedBodyLifecycleSession, c::Plan, c::HostId, c::BootId) {
    let plan = core_fixture::seal(core_fixture::fragment());
    let fragment = &plan.fragments[0];
    let host = fragment.host_id.clone();
    let boot = fragment.boot_id.clone();
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "birth".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Local".into()).unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("local-installed-image").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "admitted".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: c::OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            "present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    (
        SealedBodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap(),
        plan,
        host,
        boot,
    )
}
fn verify(plan: c::Plan) -> c::VerifiedPlanMetadata {
    c::verify_plan_metadata_with_configurations(plan, |_| Ok::<_, ()>(vec![])).unwrap()
}
#[test]
fn only_current_proposed_fragment_can_supply_local_face_provenance() {
    let (mut owner, plan, host, boot) = fixture();
    let plot = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let verified = verify(plan);
    let id = &verified.preparation_metadata().fragments[0].fragment_id;
    let active = c::bind_active_play(verified.plan_id(), &host, &boot, 1);
    assert_eq!(
        local_scope::check(&owner, &verified, id, &active),
        Err("local-face-no-proposal")
    );
    owner
        .propose_verified(
            &[VerifiedBodyPlotPlan {
                plot,
                plan: &verified,
            }],
            &[],
            None,
            &host,
            &boot,
        )
        .unwrap();
    let current = owner.realization().unwrap();
    assert_eq!(
        local_scope::check(&owner, &verified, id, &active),
        Ok((current.plan.body_id(), current.plan.wake_id()))
    );
    assert!(current.play.is_none());
    assert_eq!(
        local_scope::check(&owner, &verified, &"missing".into(), &active),
        Err("local-face-fragment")
    );
    let mut forged = active.clone();
    forged.active_play_id = "claimed".into();
    assert_eq!(
        local_scope::check(&owner, &verified, id, &forged),
        Err("local-face-current-member")
    );
    for (h, b, sequence) in [
        (&host, &boot, 0),
        (&host, &c::BootId::from("stale"), 1),
        (&c::HostId::from("foreign"), &boot, 1),
    ] {
        let stale = c::bind_active_play(verified.plan_id(), h, b, sequence);
        assert_eq!(
            local_scope::check(&owner, &verified, id, &stale),
            Err("local-face-current-member")
        );
    }
    let mut different = core_fixture::fragment();
    different.sign_storage_budget.item_capacity += 1;
    let unproposed = verify(core_fixture::seal(different));
    let unproposed_id = &unproposed.preparation_metadata().fragments[0].fragment_id;
    let unproposed_active = c::bind_active_play(unproposed.plan_id(), &host, &boot, 1);
    assert_eq!(
        local_scope::check(&owner, &unproposed, unproposed_id, &unproposed_active),
        Err("local-face-unproposed-plan")
    );
    // Preparation of publication provenance does not record a Body Play.
    assert!(owner.realization().unwrap().play.is_none());
    owner.lull(&host, &boot, None).unwrap();
    assert_eq!(
        local_scope::check(&owner, &verified, id, &active),
        Err("local-face-no-proposal")
    );
}

#[test]
fn environment_body_cannot_start_by_silently_omitting_its_display_plot() {
    use conduit_birth_plot::BirthPlotChoice;
    use conduit_environment_make::PreparedBodyImage;
    let (owner, plan, host, boot) = fixture();
    let sampler = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let display = ResidentPlot::new("source/display".into(), "checked/display".into());
    let image = PreparedBodyImage::prepare(
        "01234567-89ab-4cde-8fab-0123456789ab".into(),
        [sampler.clone(), display]
            .into_iter()
            .map(|plot| BirthPlotChoice {
                title: "work".into(),
                search_text: "work".into(),
                plot,
                refusal: None,
                selected: true,
            })
            .collect(),
        1,
        "birth/image".into(),
    )
    .unwrap();
    assert_eq!(image.evidence().body.workset.len(), 2);
    // Preserve the actual image workset while using ordinary admitted membership
    // from this fixture. The resulting Body is born with both required plots.
    let mut body = owner.evidence().clone();
    let revision = body.body.workload_revision;
    let mut owner = SealedBodyLifecycleSession::open_admitted(body.clone(), &host, &boot).unwrap();
    let display = image
        .evidence()
        .body
        .workset
        .plots()
        .iter()
        .find(|p| **p != sampler)
        .unwrap();
    owner
        .admit_plot(revision, display.clone(), &host, &boot)
        .unwrap();
    body = owner.evidence().clone();
    let verified = verify(plan);
    assert_eq!(
        owner.propose_verified(
            &[VerifiedBodyPlotPlan {
                plot: sampler,
                plan: &verified
            }],
            &[],
            None,
            &host,
            &boot
        ),
        Err(BodyLifecycleSessionError::PlanSeal(
            BodyPlanSealRefusal::Invalid(BodyPlanError::MissingPlot)
        ))
    );
    assert_eq!(owner.evidence(), &body);
    assert!(owner.realization().is_none());
}
