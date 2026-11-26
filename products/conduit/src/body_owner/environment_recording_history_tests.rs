//! Product selected IMAGE → actual service worker → Owner Face → native scene.
//! CPU preparation only: no sensor, browser membership, Show, ACK or release claim.
use super::*;
use conduit_face::{
    FacePropertyValue as V, FaceRole, PreparedQuantitySeries, QUANTITY_SERIES_ROLE,
};
use conduit_std_host::StdHostConfig;
use conduitos::native_face_scene::NativeFaceScene;
use std::{
    fs,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

fn installation() -> (PathBuf, StdHostConfig) {
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "environment-owner",
        "cpu",
    ));
    fs::create_dir_all(&root).unwrap();
    let installation = super::super::super::Installation {
        schema: super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/environment-owner-fixture".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: super::super::super::digest(b"bundle"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: None,
        selected_todo_checkpoint: None,
    };
    super::super::super::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let config = StdHostConfig {
        host_id: installation.host_id.into(),
        boot_id: "boot/environment-owner-fixture".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    };
    (root, config)
}

#[test]
fn original_recording_source_reaches_actual_owner_and_ordinary_native_scene() {
    let (root, current) = installation();
    // Root's test-only helper uses the actual product Selection/checked IMAGE
    // initializer with this exact Host/Boot, then removes the original records.
    let host = crate::durable_host::environment_recording_history_host_fixture(current.clone());
    const SOURCE: &str =
        include_str!("../../../../plots/environment-recording-history/main.conduit");
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let expanded = source.expand_entry().unwrap();
    let resident = ResidentPlot::new(
        expanded.source_document_id.clone(),
        expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(host, resident, None, "Original recording history").unwrap();
    let sources = state::WorksetSources::checked(
        owner.session.evidence(),
        vec![(SOURCE.into(), "environment-recording-history".into())],
    )
    .unwrap();
    state::retain_session_with_workset_sources(&root, &mut owner.session, &sources, None, None)
        .unwrap();
    let body = owner.session.evidence().body_id.clone();
    let mut worker = owner.start_service_run(&root, 30_000, false, 0).unwrap();
    let until = Instant::now() + Duration::from_secs(15);
    let (face, play, partition_plan) = loop {
        let retired = worker.progress(&mut owner, &root).unwrap();
        if let Some(contribution) = owner.runtime_face.as_ref() {
            assert!(owner.runtime_face_scope_matches_current(contribution));
            assert!(!contribution.scope().is_retired());
            assert_eq!(contribution.scope().host(), &current.host_id);
            assert_eq!(contribution.scope().boot(), &current.boot_id);
            assert_eq!(contribution.scope().body_play().body_id, body);
            break (
                owner.local_face_snapshot().unwrap(),
                contribution.scope().body_play().clone(),
                contribution.scope().partition_plan().clone(),
            );
        }
        assert!(!retired, "service retired before an actual contribution");
        assert!(
            Instant::now() < until,
            "service did not publish a current contribution"
        );
        thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(owner.current_play_id(), Some(&play.active_play_id));
    assert_eq!(
        owner.session.realization().unwrap().plan.plan_id,
        play.plan_id
    );
    assert!(owner
        .session
        .realization()
        .unwrap()
        .plan
        .plots
        .iter()
        .any(|plot| plot.plan.plan_id == partition_plan));
    assert_eq!(face.basis.body_id.as_ref(), Some(&body));
    let provenance = face
        .properties
        .iter()
        .find(|property| {
            property.subject.starts_with("contribution/inspection/")
                && property.name == "active-play-id"
                && matches!(&property.value, V::Identity(id) if id == play.active_play_id.as_str())
        })
        .expect("actual contribution Play provenance absent");
    for (name, expected) in [
        ("plan-id", play.plan_id.as_str()),
        ("checked-plot-id", expanded.checked_plot_id.as_str()),
    ] {
        assert!(face
            .properties
            .iter()
            .any(|property| property.subject == provenance.subject
                && property.name == name
                && matches!(&property.value, V::Identity(id) if id == expected)));
    }
    // Reported producer scope is preserved as data and differs from inspection.
    let subjects = face.subjects.iter().filter(|subject| matches!(&subject.role, FaceRole::Semantic(role) if role.as_str() == QUANTITY_SERIES_ROLE)).collect::<Vec<_>>();
    assert_eq!(subjects.len(), 2);
    for subject in subjects {
        assert!(matches!(
            subject.identity.as_str(),
            "channel/t" | "channel/h"
        ));
        let series = PreparedQuantitySeries::prepare(&face, &subject.identity, 64).unwrap();
        assert_eq!(
            series.producer(),
            [
                "host/reported",
                "boot/reported",
                "plan/reported",
                "play/reported"
            ]
        );
        assert_ne!(series.producer()[0], current.host_id.as_str());
        assert_ne!(series.producer()[1], current.boot_id.as_str());
        assert_ne!(series.producer()[2], play.plan_id.as_str());
        assert_ne!(series.producer()[3], play.active_play_id.as_str());
        let samples = series.samples();
        assert_eq!(
            samples.len(),
            2,
            "retain every original reading including failure"
        );
        assert_eq!((samples[0].sequence, samples[0].boot_ms), (1, 7));
        assert_eq!((samples[1].sequence, samples[1].boot_ms), (4, 101));
        assert!(samples[0].value.is_some());
        assert_eq!(samples[1].value, None);
        assert_eq!(samples[1].unavailable_reason, Some("checksum"));
        assert_eq!(samples[1].gap_before, Some((2, 3)));
        let projection = series.project(None, 16).unwrap();
        assert_eq!(projection.points.len(), 1);
        assert_eq!(projection.source_samples, 2);
        assert_eq!(projection.unavailable_samples, 1);
        assert_eq!(projection.gap_runs, 1);
        assert_eq!(projection.omitted_samples, "0");
        assert!(
            projection.publication_evidence.is_none() && projection.publication_reference.is_none()
        );
    }
    // Export only this genuine unchanged Owner Face for an optional later Mask
    // consumer. No Body identity is added, no fixture Face is substituted/resealed.
    if let Some(path) = std::env::var_os("CONDUIT_ENVIRONMENT_HISTORY_OWNER_FACE") {
        let path = PathBuf::from(path);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, serde_json::to_vec(&face).unwrap()).unwrap();
    }
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    assert_eq!(scene.face(), &face);
    let mut rendered = String::new();
    let mut pages = 0;
    for details in [false, true] {
        scene.show_details(details);
        while scene.page() > 0 {
            scene.turn_page(false);
        }
        for page in 0..scene.pages() {
            assert_eq!(scene.page(), page);
            let frame = scene.frame().unwrap();
            assert_eq!(frame.face_id, &face.identity);
            assert_eq!(frame.revision, face.revision);
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            for command in frame.scene.commands() {
                rendered.push_str(command.payload());
            }
            pages += 1;
            scene.turn_page(true);
        }
    }
    assert!(pages > 0);
    assert!(rendered.contains("Temperature") && rendered.contains("Relative humidity"));
    assert_eq!(scene.face(), &face);
    worker.request_lull().unwrap();
    let until = Instant::now() + Duration::from_secs(15);
    while !worker.progress(&mut owner, &root).unwrap() {
        assert!(
            Instant::now() < until,
            "service failed to retire after ordinary Lull"
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert!(owner.current_play_id().is_none());
    assert!(owner.current_home_fragment().is_none());
    assert!(owner.session.realization().is_none());
    assert_eq!(
        owner.session.evidence().body.state,
        conduit_body::BodyState::Lulled
    );
    assert_eq!(state::load(&root).unwrap().unwrap().body_id, body);
    let report = owner.last_execution.as_ref().unwrap();
    assert!(
        report["failure"].is_null() && report["cleanup_failure"].is_null(),
        "{report}"
    );
    assert_eq!(
        report["terminal"],
        serde_json::json!(conduit_core::TerminalDisposition::Cancelled {
            reason: conduit_core::CancellationReason::OperatorRequested
        })
    );
    fs::remove_dir_all(root).unwrap();
}
