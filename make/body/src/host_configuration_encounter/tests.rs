//! CPU acknowledged-Mask fixture only; no native rendering, Make or initialization.
use super::*;
use conduit_face::*;
#[path = "../../tests/common/face_mask.rs"]
mod face_mask;
fn basis() -> HostConfigurationEncounterBasis {
    let plan = conduit_patchbay_workbench_conformance::patchbay_mask_plans()
        .unwrap()
        .direct;
    let body = conduit_body::Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        "sign/host-authoring-fixture".into(),
    )
    .unwrap();
    let (_, wake) = body.wake(2, "sign/host-authoring-wake".into()).unwrap();
    HostConfigurationEncounterBasis {
        host_id: "host/fixture".into(),
        boot_id: "boot/fixture".into(),
        encounter_id: "fixture/host-authoring".into(),
        execution: FaceBasis {
            body_id: Some(body.body_id),
            wake_id: Some(wake.wake_id),
            source_document_id: Some(plan.source_document_id),
            checked_plot_id: Some(plan.checked_plot_id),
            expanded_plot_id: Some(plan.expanded_plot_id),
            plan_id: Some(plan.plan_id),
            active_play_id: Some("play/fixture".into()),
            sign_ids: vec![],
        },
    }
}
fn input(
    e: &HostConfigurationEncounter<'_>,
    b: &HostConfigurationEncounterBasis,
    id: &str,
    value: Option<&str>,
) -> (MaskShow, FaceInteraction) {
    let face = e.face(b).unwrap();
    let show = face_mask::acknowledged_fixture_show(&face);
    let action = face
        .actions
        .iter()
        .find(|a| a.identity == id)
        .unwrap_or_else(|| panic!("missing action {id}"));
    let args = value
        .map(|v| {
            vec![FaceInteractionArgument {
                name: "value".into(),
                value_kind: UTF8_TEXT_VALUE_KIND.into(),
                value: v.as_bytes().to_vec(),
            }]
        })
        .unwrap_or_default();
    let interaction = FaceInteraction::new(&face, &show, id, &action.target, args, 1).unwrap();
    (show, interaction)
}
fn act(
    e: &mut HostConfigurationEncounter<'_>,
    b: &HostConfigurationEncounterBasis,
    id: &str,
    value: Option<&str>,
) -> Result<HostConfigurationEncounterOutcome, String> {
    let (show, input) = input(e, b, id, value);
    e.apply_interaction(b, &show, &input)
}
fn source() -> &'static str {
    include_str!("../../../../targets/std/profiles/linux-computer.host.conduit")
}
#[test]
fn saved_host_configuration_roundtrips_with_original_basis_and_bounded_source_chunks() {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let mut e = HostConfigurationEncounter::load(source(), &catalog, &packages).unwrap();
    let b = basis();
    let expected = e.canonical_source().unwrap();
    assert_eq!(e.face(&b).unwrap().basis, b.execution);
    assert!(matches!(
        act(&mut e, &b, "host-config/check", None).unwrap(),
        HostConfigurationEncounterOutcome::Checked(_)
    ));
    act(&mut e, &b, "host-config/source", None).unwrap();
    let face = e.face(&b).unwrap();
    assert!(face.text.iter().all(|t| t.text.len() <= 1024));
    let chunks: Vec<_> = face.text.iter().skip(1).map(|t| t.text.as_str()).collect();
    assert_eq!(chunks.concat(), expected);
    assert!(chunks.iter().all(|c| c.len() <= 768));
}
#[test]
fn scratch_target_and_explicit_unrelated_base_and_limits_produce_checked_source() {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let b = basis();
    let mut e = HostConfigurationEncounter::new(&catalog, &packages).unwrap();
    let index = packages
        .target_descriptors()
        .iter()
        .position(|t| t.key() == "std/x86_64/computer")
        .unwrap();
    // Navigate ordinary bounded pages rather than invoking a hidden choice.
    for _ in 0..index / PAGE_SIZE {
        act(&mut e, &b, "host-config/next", None).unwrap();
    }
    act(&mut e, &b, &format!("host-config/target/{index}"), None).unwrap();
    assert!(e.checked().is_err());
    act(&mut e, &b, "host-config/name", Some("authored-host")).unwrap();
    let saved = conduit_host_make::parse_host_configuration_conduit(source()).unwrap();
    for key in draft::LIMITS {
        act(
            &mut e,
            &b,
            &format!("host-config/limit/{key}"),
            Some(&draft::limit(&saved.limits, key).to_string()),
        )
        .unwrap();
    }
    act(&mut e, &b, "host-config/bases", None).unwrap();
    let index = draft::offers(&e)
        .unwrap()
        .iter()
        .position(|o| o.offer.base_kind == "clock/monotonic")
        .unwrap();
    for _ in 0..index / PAGE_SIZE {
        act(&mut e, &b, "host-config/next", None).unwrap();
    }
    act(&mut e, &b, &format!("host-config/add/{index}"), None).unwrap();
    let checked = e.checked().unwrap();
    assert_eq!(checked.configuration().bases[0].kind, "clock/monotonic");
    assert_eq!(checked.configuration().bases[0].initialization, None);
    let canonical = e.canonical_source().unwrap();
    let loaded = HostConfigurationEncounter::load(&canonical, &catalog, &packages).unwrap();
    assert_eq!(
        checked.configuration_id(),
        loaded.checked().unwrap().configuration_id()
    );
}
#[test]
fn initialization_is_explicit_and_checked_with_no_defaults_or_partial_success() {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let b = basis();
    let mut e = HostConfigurationEncounter::load(source(), &catalog, &packages).unwrap();
    act(&mut e, &b, "host-config/bases", None).unwrap();
    let index = draft::offers(&e)
        .unwrap()
        .iter()
        .position(|o| o.offer.implementation_id == "std/alsa-microphone-clip@1")
        .unwrap();
    for _ in 0..index / PAGE_SIZE {
        act(&mut e, &b, "host-config/next", None).unwrap();
    }
    act(&mut e, &b, &format!("host-config/add/{index}"), None).unwrap();
    let selected = e.configuration().unwrap().bases.len() - 1;
    assert!(e.configuration().unwrap().bases[selected]
        .initialization
        .is_none());
    act(
        &mut e,
        &b,
        &format!("host-config/initialize/{selected}"),
        None,
    )
    .unwrap();
    act(&mut e, &b, "host-config/field/device", Some("0")).unwrap();
    assert!(e.checked().is_err());
    let before = e.configuration().unwrap().clone();
    assert!(act(
        &mut e,
        &b,
        "host-config/field/capture_milliseconds",
        Some("7000")
    )
    .is_err());
    assert_eq!(e.configuration().unwrap(), &before);
    for (key, value) in [
        ("executable", "/usr/bin/arecord"),
        (
            "executable_sha256",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ),
        ("card_id", "CMTECK"),
        ("base_identity", "fixture/microphone"),
        ("capture_milliseconds", "3000"),
        ("timeout_milliseconds", "10000"),
    ] {
        act(&mut e, &b, &format!("host-config/field/{key}"), Some(value)).unwrap();
    }
    e.checked().unwrap();
    act(&mut e, &b, "host-config/clear-initialization", None).unwrap();
    assert!(e.configuration().unwrap().bases[selected]
        .initialization
        .is_none());
}
#[test]
fn stale_interaction_wrong_basis_unacknowledged_show_and_finite_bounds_refuse() {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let b = basis();
    let mut e = HostConfigurationEncounter::load(source(), &catalog, &packages).unwrap();
    let (show, old) = input(&e, &b, "host-config/name", Some("old"));
    act(&mut e, &b, "host-config/name", Some("current")).unwrap();
    assert!(e.apply_interaction(&b, &show, &old).is_err());
    let face = e.face(&b).unwrap();
    let show = face_mask::prepared_fixture_show(&face);
    let action = face
        .actions
        .iter()
        .find(|a| a.identity == "host-config/name")
        .unwrap();
    assert!(FaceInteraction::new(
        &face,
        &show,
        &action.identity,
        &action.target,
        vec![FaceInteractionArgument {
            name: "value".into(),
            value_kind: UTF8_TEXT_VALUE_KIND.into(),
            value: b"unacknowledged".to_vec()
        }],
        1
    )
    .is_err());
    let mut invalid = basis();
    invalid.execution.active_play_id = None;
    assert!(e.face(&invalid).is_err());
    assert!(HostConfigurationEncounter::load(
        &"x".repeat(MAX_HOST_ENCOUNTER_SOURCE_BYTES + 1),
        &catalog,
        &packages
    )
    .is_err());
    assert!(HostConfigurationEncounter::load(
        &source().replace("machine: \"computer\"", "machine: \"unknown-machine\""),
        &catalog,
        &packages
    )
    .is_err());
    e.interactions = MAX_HOST_ENCOUNTER_INTERACTIONS;
    assert!(act(&mut e, &b, "host-config/name", Some("overflow")).is_err());
    assert_eq!(e.configuration().unwrap().name, "current");
}

#[test]
fn choices_are_canonical_bounded_collections_and_unicode_diagnostics_fit_face() {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let mut e = HostConfigurationEncounter::new(&catalog, &packages).unwrap();
    let b = basis();
    let face = e.face(&b).unwrap();
    let collection = prepare_face_collection(&face, "host-config/choices", PAGE_SIZE).unwrap();
    assert!(collection.items().len() <= PAGE_SIZE);
    let mut c = conduit_host_make::parse_host_configuration_conduit(source()).unwrap();
    c.bases[0].implementation = Some("界".repeat(200));
    e.configuration = Some(c);
    e.page = Page::Main;
    let face = e.face(&b).unwrap();
    assert!(face.text.iter().all(|t| t.text.len() <= 1024));
    let input = draft::bounded_text(&"界".repeat(1000), 768);
    assert!(input.len() <= 768);
}

#[test]
fn saved_explicit_capabilities_survive_name_edit_and_check_and_target_refuses_without_loss() {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let source =
        include_str!("../../../../targets/std/profiles/raspberry-pi-3-tick-workset.host.conduit");
    let mut encounter = HostConfigurationEncounter::load(source, &catalog, &packages).unwrap();
    let b = basis();
    let before = encounter.configuration().unwrap().capabilities.clone();
    assert!(matches!(
        act(&mut encounter, &b, "host-config/check", None).unwrap(),
        HostConfigurationEncounterOutcome::Checked(_)
    ));
    act(&mut encounter, &b, "host-config/name", Some("renamed-pi")).unwrap();
    assert_eq!(encounter.configuration().unwrap().capabilities, before);
    let canonical = encounter.canonical_source().unwrap();
    assert!(act(&mut encounter, &b, "host-config/target/0", None).is_err());
    assert_eq!(encounter.canonical_source().unwrap(), canonical);
    assert_eq!(encounter.configuration().unwrap().capabilities, before);
}
