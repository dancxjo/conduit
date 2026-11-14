//! Actual ordinary Crèche kernel/BodyPlay with explicitly declared CPU carrier
//! evidence. This proves route custody, not a browser socket or physical Show.
use super::*;
use conduit_body::{
    PortableSpawnAdmissionRequest, SpawnInvitationSecret, SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::{HostAdvertisement, LineOffer, LinkAuthorityReference, LinkCredentialReference};
use conduit_face::{RemoteOwnerMaskRouteError, RemoteOwnerMaskRouteSeal};

fn browser(owner: &mut Owner, root: &std::path::Path) -> HostAdvertisement {
    let mut browser = conduit_std_host::StdHost::new_with_config(StdHostConfig {
        host_id: "host/plot-route-browser".into(),
        boot_id: "boot/plot-route-browser".into(),
        offer_generation: OfferGeneration(1),
    })
    .advertisement()
    .clone();
    browser
        .capabilities
        .push(conduit_browser_mask_offer::offer());
    if !browser
        .resources
        .iter()
        .any(|r| r.class_id.as_str() == conduit_core::FACE_RESOURCE_CLASS)
    {
        browser.resources.push(conduit_core::resource_offer(
            "browser/face",
            conduit_core::FACE_RESOURCE_CLASS,
            1,
        ));
    }
    browser
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    browser.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let invitation = owner.issue_invitation(root, 60, None).unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: browser.clone(),
        nonce: invitation.claim.nonce,
        signature: secret
            .sign(&invitation.claim.signing_transcript(
                &browser.host_id,
                &browser.boot_id,
                browser.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    owner
        .admit_invited(root, request, browser.host_id.as_str())
        .unwrap();
    browser
}
fn line(name: &str, source: &HostAdvertisement, sink: &HostAdvertisement) -> LineOffer {
    let d = &conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0];
    let mut line = conduit_core::process_owned_line_offer_with_limits(
        name,
        &format!("binding/{name}"),
        d.base_implementation_id.into(),
        "instance/fixture",
        source,
        sink,
        conduit_core::LinkLimits {
            maximum_in_flight_items: d.maximum_in_flight_items,
            maximum_payload_bytes: d.maximum_payload_bytes,
            maximum_buffered_bytes: d.maximum_buffered_bytes,
            maximum_frame_bytes: d.maximum_frame_bytes,
        },
    );
    line.contract = d.contract;
    line.binding.authority = LinkAuthorityReference::Grant(format!("grant/{name}").as_str().into());
    line.binding.credential = LinkCredentialReference::Opaque("credential/cpu-carrier".into());
    line
}

#[test]
fn remote_mask_preserves_actual_plot_producer_and_rejects_foreign_or_ended_execution() {
    let fixture = Fixture::new();
    let factory = fixture.factory();
    let mut owner = fixture.owner(Some(factory.resource_offer()));
    owner.install_creche_factory(factory).unwrap();
    let browser = browser(&mut owner, &fixture.root);
    let mut worker = owner.start_service_run(&fixture.root, 10_000).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let face = loop {
        assert!(Instant::now() < deadline);
        assert!(!worker.progress(&mut owner, &fixture.root).unwrap());
        let face = owner.local_face_snapshot().unwrap();
        if face.basis.active_play_id.is_some() {
            break face;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let local = owner.host.advertisement().clone();
    let face_line = line("line/face", &local, &browser);
    let returned = line("line/return", &browser, &local);
    let interaction = line("line/interaction", &browser, &local);
    let planned = conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
        &browser,
        &local,
        &face_line,
        &returned,
        &interaction,
        conduit_browser_mask_offer::MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    let seal = |face: &conduit_face::Face| {
        RemoteOwnerMaskRouteSeal::seal_current_with_interaction(
            &owner.session,
            face,
            &local,
            &browser,
            &planned,
            &face_line,
            &returned,
            &interaction,
        )
    };
    let route = seal(&face).unwrap();
    assert_eq!(route.face_basis, face.basis);
    route.verify_seal().unwrap();
    route
        .validate_current_with_interaction(
            &owner.session,
            &face,
            &local,
            &browser,
            &face_line,
            &returned,
            &interaction,
        )
        .unwrap();
    let mut variants = Vec::new();
    let mut basis = face.basis.clone();
    basis.plan_id = Some(owner.session.realization().unwrap().plan.plan_id.clone());
    variants.push(basis); // A BodyPlan is not the producing PlotPlan.
    let mut basis = face.basis.clone();
    basis.active_play_id = Some("play/foreign".into());
    variants.push(basis);
    let mut basis = face.basis.clone();
    basis.source_document_id = Some("source/foreign".into());
    variants.push(basis);
    let mut basis = face.basis.clone();
    basis.checked_plot_id = Some("checked/foreign".into());
    variants.push(basis);
    let mut basis = face.basis.clone();
    basis.expanded_plot_id = Some("expanded/foreign".into());
    variants.push(basis);
    let mut basis = face.basis.clone();
    basis.active_play_id = None;
    variants.push(basis);
    for basis in variants {
        assert_eq!(
            seal(&face.clone().with_basis(basis).unwrap()),
            Err(RemoteOwnerMaskRouteError::WrongFaceBasis)
        );
    }
    worker.request_lull().unwrap();
    while !worker.progress(&mut owner, &fixture.root).unwrap() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(route
        .validate_current_with_interaction(
            &owner.session,
            &face,
            &local,
            &browser,
            &face_line,
            &returned,
            &interaction
        )
        .is_err());
}
