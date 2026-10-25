//! Interaction conformance using an acknowledged Mask fixture, not native proof.
use conduit_body::Body;
use conduit_body_make::*;
use conduit_face::*;
use conduit_workspace_make::package_set;
#[path = "common/face_mask.rs"]
mod face_mask;
#[path = "common/deployment.rs"]
mod fixture;

fn basis() -> DeploymentEncounterBasis {
    let plan = conduit_patchbay_workbench_conformance::patchbay_mask_plans()
        .unwrap()
        .direct;
    let body = Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "sign/fixture/birth".into(),
    )
    .unwrap();
    DeploymentEncounterBasis {
        host_id: "host/fixture".into(),
        boot_id: "boot/fixture".into(),
        encounter_id: "fixture/deployment".into(),
        execution: FaceBasis {
            body_id: Some(body.body_id),
            wake_id: None,
            source_document_id: Some(plan.source_document_id),
            checked_plot_id: Some(plan.checked_plot_id),
            expanded_plot_id: Some(plan.expanded_plot_id),
            plan_id: Some(plan.plan_id),
            active_play_id: None,
            sign_ids: vec![],
        },
    }
}
fn input(
    e: &DeploymentEncounter,
    b: &DeploymentEncounterBasis,
    action: &str,
    value: Option<&[u8]>,
) -> (MaskShow, FaceInteraction) {
    let face = e.face(b).unwrap();
    let show = face_mask::acknowledged_fixture_show(&face);
    let target = &face
        .actions
        .iter()
        .find(|a| a.identity == action)
        .unwrap()
        .target;
    let args = value
        .map(|v| {
            vec![FaceInteractionArgument {
                name: "value".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: v.to_vec(),
            }]
        })
        .unwrap_or_default();
    let interaction = FaceInteraction::new(&face, &show, action, target, args, 1).unwrap();
    (show, interaction)
}
fn act(
    e: &mut DeploymentEncounter,
    b: &DeploymentEncounterBasis,
    action: &str,
    value: Option<&[u8]>,
) -> DeploymentEncounterOutcome {
    let (show, interaction) = input(e, b, action, value);
    e.apply_interaction(
        b,
        &show,
        &interaction,
        &conduit_workspace_make::catalog(),
        &package_set(),
    )
    .unwrap()
}
#[test]
fn canonical_encounter_retains_edited_name_hosts_scope_and_source_into_review() {
    let b = basis();
    let mut e = DeploymentEncounter::new(fixture::encounter("Garden"));
    let face = e.face(&b).unwrap();
    assert_eq!(face.basis.body_id, b.execution.body_id);
    assert!(!face.actions.iter().any(|a| a.identity == "creche.birth"));
    act(&mut e, &b, "creche.name", Some(b"Willow"));
    act(&mut e, &b, "creche.deploy.next", None);
    assert_eq!(e.page(), DeploymentPage::Hosts);
    act(&mut e, &b, "creche.deploy.first", Some(b"second"));
    act(&mut e, &b, "creche.deploy.scope", Some(b"all"));
    act(&mut e, &b, "creche.deploy.next", None);
    act(&mut e, &b, "creche.deploy.source", Some(b"build"));
    let DeploymentEncounterOutcome::Reviewed(review) =
        act(&mut e, &b, "creche.deploy.review", None)
    else {
        panic!("expected review")
    };
    let prepared = e
        .into_draft()
        .prepare(
            review,
            1,
            "sign/fixture/deployment".into(),
            &conduit_workspace_make::catalog(),
            &package_set(),
        )
        .unwrap();
    assert_eq!(prepared.birth().evidence().friendly_name, "Willow");
    assert_eq!(prepared.selected_hosts(), ["first", "second"]);
    assert_eq!(prepared.first_host(), "second");
    assert_eq!(prepared.image_source(), DeploymentImageSource::SourceBuild);
}
#[test]
fn old_show_cannot_navigate_or_edit_after_a_new_face() {
    let b = basis();
    let mut e = DeploymentEncounter::new(fixture::encounter("Garden"));
    let (show, old) = input(&e, &b, "creche.deploy.next", None);
    act(&mut e, &b, "creche.name", Some(b"Changed"));
    assert!(matches!(
        e.apply_interaction(
            &b,
            &show,
            &old,
            &conduit_workspace_make::catalog(),
            &package_set()
        ),
        Err(DeploymentEncounterRefusal::Interaction(
            FaceInteractionRefusal::StaleFace
        ))
    ));
    assert_eq!(e.page(), DeploymentPage::Body);
}
#[test]
fn out_of_catalog_host_is_rejected_by_the_exact_action_contract() {
    let b = basis();
    let mut e = DeploymentEncounter::new(fixture::encounter("Garden"));
    act(&mut e, &b, "creche.deploy.next", None);
    let (show, mut interaction) = input(&e, &b, "creche.deploy.first", Some(b"first"));
    interaction.arguments[0].value = b"foreign".to_vec();
    assert!(e
        .apply_interaction(
            &b,
            &show,
            &interaction,
            &conduit_workspace_make::catalog(),
            &package_set()
        )
        .is_err());
    assert_eq!(e.draft().first_host(), "first");
}
#[test]
fn existing_body_and_complete_producer_identity_are_required() {
    let mut b = basis();
    let e = DeploymentEncounter::new(fixture::encounter("Garden"));
    b.execution.body_id = None;
    assert!(matches!(
        e.face(&b),
        Err(DeploymentEncounterRefusal::InvalidBasis)
    ));
    b = basis();
    b.execution.plan_id = None;
    assert!(matches!(
        e.face(&b),
        Err(DeploymentEncounterRefusal::InvalidBasis)
    ));
}

#[test]
fn acknowledged_host_name_and_profile_edits_are_checked_and_isolated() {
    let b = basis();
    let draft = fixture::encounter("Garden");
    let original = draft.construction().hosts()[0].configuration.clone();
    let alternate = fixture::encounter_with_memory("Garden", 32 * 1024 * 1024)
        .construction()
        .hosts()[0]
        .configuration
        .clone();
    let alternate_id = alternate.configuration_id().to_owned();
    let mut e = DeploymentEncounter::with_profiles(draft, vec![original, alternate]).unwrap();
    act(&mut e, &b, "creche.deploy.next", None);
    act(&mut e, &b, "creche.deploy.host-name.0", Some(b"garden-pi"));
    assert_eq!(e.draft().first_host(), "garden-pi");
    act(
        &mut e,
        &b,
        "creche.deploy.profile.0",
        Some(alternate_id.as_bytes()),
    );
    let hosts = e.draft().construction().hosts();
    assert_eq!(hosts[0].description.name, "garden-pi");
    assert_eq!(
        hosts[0]
            .configuration
            .configuration()
            .limits
            .heap_arena_bytes,
        32 * 1024 * 1024
    );
    assert_eq!(
        hosts[1]
            .configuration
            .configuration()
            .limits
            .heap_arena_bytes,
        64 * 1024 * 1024
    );
    assert_ne!(
        hosts[0].description.configuration,
        hosts[1].description.configuration
    );
}

#[test]
fn target_change_selects_the_target_packages_exact_default_media() {
    let b = basis();
    let draft = fixture::encounter("Garden");
    let original = draft.construction().hosts()[0].configuration.clone();
    let source =
        include_str!("../../../targets/conduitos/profiles/conduitos-x86_64-pc.host.conduit");
    let pc = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(source).unwrap(),
        &conduit_workspace_make::catalog(),
        &package_set(),
    )
    .unwrap();
    let pc_id = pc.configuration_id().to_owned();
    let target = pc.profile().target.key();
    let expected_output = package_set()
        .target_descriptor(&target)
        .unwrap()
        .default_output
        .clone();
    let mut e = DeploymentEncounter::with_profiles(draft, vec![original, pc]).unwrap();
    act(&mut e, &b, "creche.deploy.next", None);
    act(
        &mut e,
        &b,
        "creche.deploy.profile.0",
        Some(pc_id.as_bytes()),
    );
    let host = &e.draft().construction().hosts()[0];
    assert_eq!(host.configuration.profile().target.key(), target);
    assert_eq!(host.description.spore.output, expected_output);
}

#[test]
fn unfinished_body_name_disables_forward_commitment() {
    let b = basis();
    let mut e = DeploymentEncounter::new(fixture::encounter("Garden"));
    act(&mut e, &b, "creche.name", Some(b""));
    let face = e.face(&b).unwrap();
    assert!(!face
        .actions
        .iter()
        .find(|a| a.identity == "creche.deploy.next")
        .unwrap()
        .availability
        .is_available());
}

#[test]
fn catalogue_must_include_the_current_exact_profiles_and_stay_finite() {
    let draft = fixture::encounter("Garden");
    assert!(matches!(
        DeploymentEncounter::with_profiles(draft, vec![]),
        Err(DeploymentEncounterRefusal::InvalidCatalogue)
    ));
    let draft = fixture::encounter("Garden");
    let profile = draft.construction().hosts()[0].configuration.clone();
    assert!(matches!(
        DeploymentEncounter::with_profiles(draft, vec![profile; MAX_DEPLOYMENT_PROFILES + 1]),
        Err(DeploymentEncounterRefusal::InvalidCatalogue)
    ));
}
