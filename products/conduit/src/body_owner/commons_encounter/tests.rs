use super::*;
use conduit_body::ResidentPlot;
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_face::{FaceInteractionArgument, FaceInteractionRefusal};
use conduit_std_host::{StdHost, StdHostConfig};
const CLOCK: &str = include_str!("../../../../../plots/clock/main.conduit");
fn owner(boot: &str) -> Owner {
    let plot = crate::plot_source::parse(CLOCK)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("host/commons-owner-test"),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
    });
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let born = Owner::open(host, resident.clone(), None, "Retained ticker").unwrap();
    let evidence = born.session.evidence().clone();
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("host/commons-owner-test"),
        boot_id: BootId::from(format!("retained/{boot}")),
        offer_generation: OfferGeneration(1),
    });
    let mut owner = Owner::open(host, resident, Some(evidence.clone()), "Retained ticker").unwrap();
    assert_eq!(owner.session.evidence().body_id, evidence.body_id);
    assert_eq!(owner.session.evidence().body.events, evidence.body.events);
    owner.set_resident_plot_name(&plot).unwrap();
    owner
}
fn change(face: &Face) -> &conduit_face::FaceAction {
    face.actions
        .iter()
        .find(|action| action.intent == super::super::clock_interval::CLOCK_INTERVAL_ACTION)
        .unwrap()
}
fn argument(face: &Face) -> Vec<FaceInteractionArgument> {
    let action = change(face);
    vec![FaceInteractionArgument {
        name: action.arguments[0].name.clone(),
        value_kind: action.arguments[0].contract.value_kind.as_str().into(),
        value: b"500".to_vec(),
    }]
}
fn interaction(face: &Face, show: &MaskShow) -> FaceInteraction {
    FaceInteraction::new(
        face,
        show,
        &change(face).identity,
        &change(face).target,
        argument(face),
        1,
    )
    .unwrap()
}
fn persist(owner: &mut Owner) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "commons-owner-{}-{}",
        std::process::id(),
        owner.session.evidence().body_id.as_str().replace('/', "-")
    ));
    std::fs::create_dir_all(&root).unwrap();
    // Ordinary installed owner retention, not a Commons state store.
    let installation = super::super::super::super::Installation {
        schema: super::super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/commons-owner-test".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: super::super::super::super::digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_todo_checkpoint: None,
    };
    super::super::super::super::write_json_atomic(&root.join("installation.json"), &installation)
        .unwrap();
    owner.persist(&root).unwrap();
    std::fs::write(root.join("body/source.conduit"), CLOCK).unwrap();
    root
}
#[test]
fn commons_actual_source_navigation_preserves_the_original_owner_and_direction() {
    let owner = owner("boot/commons-navigation");
    let host = owner.host.advertisement().clone();
    let before = owner.truth();
    let mut commons = CommonsEncounter::prepare().unwrap();
    let hub = commons.project(Some(&owner), &host).unwrap();
    assert_eq!(
        hub.basis.body_id.as_ref(),
        Some(&owner.session.evidence().body_id)
    );
    assert!(change(&hub).availability.is_available());
    assert_eq!(
        hub.subjects
            .iter()
            .filter(|s| matches!(&s.role, FaceRole::Semantic(id) if id.as_str() == "commons/place"))
            .count(),
        3
    );
    let workshop = commons
        .navigate(Some(&owner), &host, &hub, "commons/workshop", false)
        .unwrap();
    assert!(commons
        .navigate(Some(&owner), &host, &hub, "commons/creche", false)
        .unwrap_err()
        .contains("stale-context"));
    assert!(commons
        .navigate(Some(&owner), &host, &workshop, "commons/creche", false)
        .unwrap_err()
        .contains("source-refused"));
    assert!(commons
        .navigate(Some(&owner), &host, &workshop, "commons/hub", false)
        .unwrap_err()
        .contains("source-refused"));
    assert!(commons
        .navigate(Some(&owner), &host, &workshop, "commons/invented", false)
        .is_err());
    let returned = commons
        .navigate(Some(&owner), &host, &workshop, "commons/hub", true)
        .unwrap();
    let creche = commons
        .navigate(Some(&owner), &host, &returned, "commons/creche", false)
        .unwrap();
    assert!(!change(&creche).availability.is_available());
    let returned = commons
        .navigate(Some(&owner), &host, &creche, "commons/hub", true)
        .unwrap();
    let again = commons
        .navigate(Some(&owner), &host, &returned, "commons/workshop", false)
        .unwrap();
    assert_ne!(workshop.identity, again.identity);
    assert_eq!(workshop.basis, again.basis);
    assert_eq!(
        owner.truth(),
        before,
        "navigation must not birth, wake, or replace a Plot"
    );
    assert!(!again
        .actions
        .iter()
        .any(|a| a.intent == "conduit.intent/open-inspection@1" && a.availability.is_available()));
    for action in &again.actions {
        let actual = owner.local_face_snapshot().unwrap();
        let original = actual
            .actions
            .iter()
            .find(|a| a.identity == action.identity)
            .unwrap();
        assert_eq!(
            (
                &action.identity,
                &action.intent,
                &action.target,
                &action.arguments
            ),
            (
                &original.identity,
                &original.intent,
                &original.target,
                &original.arguments
            )
        );
    }
}
#[test]
fn commons_no_owner_and_current_life_are_observations_never_birth_or_wake() {
    let absent_host = StdHost::new();
    let host = absent_host.advertisement().clone();
    let mut commons = CommonsEncounter::prepare().unwrap();
    let none = commons.project(None, &host).unwrap();
    assert!(none.basis.body_id.is_none());
    assert!(none.actions.is_empty());
    let creche = commons
        .navigate(None, &host, &none, "commons/creche", false)
        .unwrap();
    assert!(creche.actions.is_empty());
    let hub = commons
        .navigate(None, &host, &creche, "commons/hub", true)
        .unwrap();
    assert!(hub.basis.body_id.is_none());
    // A Body is explicitly created/reattached only now, outside navigation.
    let mut owner = owner("boot/commons-lives");
    let host = owner.host.advertisement().clone();
    let lulled = commons.project(Some(&owner), &host).unwrap();
    assert!(change(&lulled).availability.is_available());
    let before_id = owner.session.evidence().body_id.clone();
    // The actual existing owner proposes its checked Plan, which wakes this
    // same Body. Commons neither stores nor performs the transition.
    let plot = crate::plot_source::parse(CLOCK)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    owner.plan(&plot).unwrap();
    let awake = commons.project(Some(&owner), &host).unwrap();
    assert!(!change(&awake).availability.is_available());
    assert_eq!(awake.basis.body_id, Some(before_id.clone()));
    assert!(awake
        .properties
        .iter()
        .any(|p| p.name == "commons/owner-life"
            && p.value == FacePropertyValue::Text("awake".into())));
    owner.lull().unwrap();
    let back = commons.project(Some(&owner), &host).unwrap();
    assert_eq!(back.basis.body_id, Some(before_id));
    assert!(change(&back).availability.is_available());
    assert!(commons
        .source
        .encounter(
            "fabricated-body",
            "none",
            0,
            "commons/hub",
            "commons/hub",
            false
        )
        .is_err());
    assert!(commons
        .source
        .encounter("", "lulled", 0, "commons/hub", "commons/hub", false)
        .is_err());
    assert!(commons
        .source
        .encounter("", "unknown", 0, "commons/hub", "commons/hub", false)
        .is_ok());
}
#[test]
fn commons_actual_acknowledged_show_routes_once_to_the_existing_owner() {
    let mut owner = owner("boot/commons-action");
    let root = persist(&mut owner);
    let original_id = owner.session.evidence().body_id.clone();
    let mut commons = CommonsEncounter::prepare().unwrap();
    let host = owner.host.advertisement().clone();
    let hub = commons.project(Some(&owner), &host).unwrap();
    commons
        .navigate(Some(&owner), &host, &hub, "commons/workshop", false)
        .unwrap();
    let (face, show) = commons
        .present(Some(&owner), &host, &mut Vec::new())
        .unwrap();
    let request = interaction(&face, &show);
    let before = owner.truth();
    let mut wrong = request.clone();
    wrong.target = "body/fabricated".into();
    let wrong_target = commons
        .apply_clock_change(&mut owner, &root, &show, &wrong)
        .unwrap_err();
    assert!(wrong_target.contains("WrongTarget"));
    let mut wrong = request.clone();
    wrong.arguments[0].value = b"3000".to_vec();
    let wrong_argument = commons
        .apply_clock_change(&mut owner, &root, &show, &wrong)
        .unwrap_err();
    assert_eq!(owner.truth(), before);
    let result = commons
        .apply_clock_change(&mut owner, &root, &show, &request)
        .unwrap();
    assert_eq!(result["interval_ms"], 500);
    assert_eq!(result["prior_show_id"], show.show_id.as_str());
    assert_eq!(owner.session.evidence().body_id, original_id);
    assert_eq!(owner.session.evidence().body.state, BodyState::Lulled);
    assert_eq!(owner.session.evidence().body.workload_revision, 2);
    assert_eq!(
        super::super::state::load(&root).unwrap().unwrap(),
        *owner.session.evidence()
    );
    let after = owner.truth();
    let replay = commons
        .apply_clock_change(&mut owner, &root, &show, &request)
        .unwrap_err();
    assert_eq!(replay, "commons/show-not-current");
    assert_eq!(owner.truth(), after);
    let next = commons.project(Some(&owner), &host).unwrap();
    assert_eq!(next.basis.body_id, Some(original_id));
    assert_ne!(next.identity, face.identity);
    println!(
        "COMMONS_OWNER_TRACE={}",
        serde_json::json!({"source":commons.source.checked.source_document_id,
        "body_id":next.basis.body_id,"prior_face":face.identity,"next_face":next.identity,
        "show_id":show.show_id,"show":show.show,"interaction":request,"result":result,"accepted_effects":1,"refusals":{"wrong_target":wrong_target,"wrong_argument":wrong_argument,"replay":replay},"before_body":before["biography"]["body"],"after_body":after["biography"]["body"]})
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn commons_refuses_replaced_owner_host_stale_show_and_unavailable_actions() {
    let mut owner = owner("boot/commons-negative");
    let root = persist(&mut owner);
    let host = owner.host.advertisement().clone();
    let mut commons = CommonsEncounter::prepare().unwrap();
    let (face, show) = commons
        .present(Some(&owner), &host, &mut Vec::new())
        .unwrap();
    let request = interaction(&face, &show);
    let before = owner.truth();
    let mut other = self::owner("boot/commons-negative");
    assert!(commons
        .apply_clock_change(&mut other, &root, &show, &request)
        .unwrap_err()
        .contains("StaleFace"));
    let mut missing = host.clone();
    missing.capabilities.clear();
    assert_eq!(
        commons.project(Some(&owner), &missing).unwrap_err(),
        "commons/owner-host-changed"
    );
    // Simulate actual Host offer withdrawal on the owner, not a Mask boolean.
    owner.host =
        super::super::OwnerHost::new(StdHost::from_advertisement(missing.clone()).unwrap());
    let unavailable = commons.project(Some(&owner), &missing).unwrap();
    assert!(!change(&unavailable).availability.is_available());
    assert!(commons
        .apply_clock_change(&mut owner, &root, &show, &request)
        .unwrap_err()
        .contains("StaleFace"));
    assert_ne!(
        owner.session.evidence().body_id,
        other.session.evidence().body_id
    );
    owner.host = super::super::OwnerHost::new(StdHost::from_advertisement(host.clone()).unwrap());
    assert_eq!(owner.truth(), before);
    commons
        .navigate(Some(&owner), &host, &face, "commons/creche", false)
        .unwrap();
    assert_eq!(
        commons
            .apply_clock_change(&mut owner, &root, &show, &request)
            .unwrap_err(),
        "commons/show-not-current"
    );
    let (creche, show) = commons
        .present(Some(&owner), &host, &mut Vec::new())
        .unwrap();
    assert_eq!(
        FaceInteraction::new(
            &creche,
            &show,
            &change(&creche).identity,
            &change(&creche).target,
            argument(&creche),
            2
        )
        .unwrap_err(),
        FaceInteractionRefusal::UnavailableAction
    );
    let inspection = creche
        .actions
        .iter()
        .find(|a| a.intent == "conduit.intent/open-inspection@1")
        .unwrap();
    assert_eq!(
        FaceInteraction::new(
            &creche,
            &show,
            &inspection.identity,
            &inspection.target,
            vec![],
            2
        )
        .unwrap_err(),
        FaceInteractionRefusal::UnavailableAction
    );
    assert_eq!(owner.truth(), before);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn commons_fulfilled_owner_and_failed_output_never_retain_action_custody() {
    struct FailedOutput;
    impl Write for FailedOutput {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("cancelled fixture output"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("cancelled fixture output"))
        }
    }
    let mut owner = owner("boot/commons-withdrawal");
    let host = owner.host.advertisement().clone();
    let before = owner.truth();
    let mut commons = CommonsEncounter::prepare().unwrap();
    let (face, show) = commons
        .present(Some(&owner), &host, &mut Vec::new())
        .unwrap();
    let request = interaction(&face, &show);
    assert!(commons
        .present(Some(&owner), &host, &mut FailedOutput)
        .is_err());
    assert!(commons.shown.is_none());
    assert_eq!(
        commons
            .apply_clock_change(&mut owner, Path::new("unused"), &show, &request)
            .unwrap_err(),
        "commons/show-not-current"
    );
    assert_eq!(owner.truth(), before);
    let id = owner.session.evidence().body_id.clone();
    owner
        .session
        .fulfill(
            &host.host_id,
            &host.boot_id,
            "grant/commons-owner-fixture".into(),
            "explicit fixture conclusion".into(),
        )
        .unwrap();
    let fulfilled = commons.project(Some(&owner), &host).unwrap();
    assert_eq!(fulfilled.basis.body_id, Some(id));
    assert!(fulfilled
        .actions
        .iter()
        .all(|a| !a.availability.is_available()));
    assert!(fulfilled
        .properties
        .iter()
        .any(|p| p.name == "commons/owner-life"
            && p.value == FacePropertyValue::Text("fulfilled".into())));
}

#[test]
fn commons_requires_current_producer_and_native_observation_bounds() {
    let mut owner = owner("boot/commons-producer");
    let before = owner.truth();
    let host = owner.host.advertisement().clone();
    let mut commons = CommonsEncounter::prepare().unwrap();
    assert!(commons
        .source
        .encounter(
            &"x".repeat(257),
            "lulled",
            0,
            "commons/hub",
            "commons/hub",
            false
        )
        .is_err());
    assert!(commons
        .source
        .encounter(
            "body/test",
            "invented",
            0,
            "commons/hub",
            "commons/hub",
            false
        )
        .is_err());
    assert_eq!(
        commons
            .source
            .encounter(
                "",
                "none",
                0,
                "commons/hub",
                &"x".repeat(conduit_core::MAXIMUM_STRUCTURED_LEAF_BYTES + 1),
                false
            )
            .unwrap_err(),
        "commons/leaf-pressure"
    );
    let (face, show) = commons
        .present(Some(&owner), &host, &mut Vec::new())
        .unwrap();
    let request = interaction(&face, &show);
    drop(commons.execution.take());
    assert_eq!(
        commons
            .apply_clock_change(&mut owner, Path::new("unused"), &show, &request)
            .unwrap_err(),
        "commons/missing-current-producer"
    );
    assert_eq!(owner.truth(), before);
    assert!(commons.shown.is_none());
}

#[path = "observation_tests.rs"]
mod current_observation;

#[path = "linear_journey.rs"]
mod linear_journey;
