//! Checked Linux IMAGE fixture for the selected Home source and Face Backs.
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, CheckedHostConfiguration, HostImage,
};
use conduit_std_offers::{
    AMBIENT_HOME_FACE_BASE, AMBIENT_HOME_FACE_DRIVER, AMBIENT_HOME_FACE_IMPLEMENTATION,
    AMBIENT_HOME_SOURCE_BASE, AMBIENT_HOME_SOURCE_IMPLEMENTATION, AMBIENT_HOME_SOURCE_RESOURCE,
    TIME_EVERY_IMPLEMENTATION,
};

fn checked_image(endpoint: std::net::SocketAddr) -> (CheckedHostConfiguration, HostImage) {
    checked_image_for(endpoint, conduit_std_offers::AmbientFaceProjection::Home)
}

fn checked_image_for(
    endpoint: std::net::SocketAddr,
    projection: conduit_std_offers::AmbientFaceProjection,
) -> (CheckedHostConfiguration, HostImage) {
    let (source_kind, source_implementation, source_resource) = match projection {
        conduit_std_offers::AmbientFaceProjection::Home => (
            AMBIENT_HOME_SOURCE_BASE,
            AMBIENT_HOME_SOURCE_IMPLEMENTATION,
            AMBIENT_HOME_SOURCE_RESOURCE,
        ),
        conduit_std_offers::AmbientFaceProjection::Clock => (
            conduit_semantic_catalog::AMBIENT_CLOCK_KIND,
            conduit_std_offers::AMBIENT_CLOCK_SOURCE_IMPLEMENTATION,
            conduit_std_offers::AMBIENT_CLOCK_SOURCE_RESOURCE,
        ),
        conduit_std_offers::AmbientFaceProjection::Wallpaper => (
            conduit_semantic_catalog::RESIDENT_WALLPAPER_KIND,
            conduit_std_offers::AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION,
            conduit_std_offers::AMBIENT_WALLPAPER_SOURCE_RESOURCE,
        ),
    };
    let face_kind = projection.kind();
    let face_implementation = projection.implementation();
    let workspace = conduit_semantic_catalog::ambient_home_workspace_bytes().unwrap();
    let source = format!(
        r#"host ambient-home-fixture {{
  schema = 1
  target = {{architecture: "x86_64", machine: "computer", os: "linux"}}
  base = {{kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}}
  base = {{kind: "{AMBIENT_HOME_SOURCE_BASE}", implementation: "{source_implementation}", initialization: {{timezone: "America/Los_Angeles", weather_endpoint: "{endpoint}", source_pool: "ambient/source", memory_pool: "ambient/worker-memory", provider_access: "operator-admitted", photo_source: "disabled"}}}}
  base = {{kind: "{AMBIENT_HOME_FACE_BASE}", implementation: "{AMBIENT_HOME_FACE_DRIVER}"}}
  capability = {{kind: "time/every", pinned_implementation: "{TIME_EVERY_IMPLEMENTATION}"}}
  capability = {{kind: "{source_kind}", pinned_implementation: "{source_implementation}"}}
  capability = {{kind: "{face_kind}", pinned_implementation: "{face_implementation}"}}
  need = {{id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}}
  need = {{id: "ambient/source", class: "{source_resource}", slots: 1, bytes: 8192}}
  need = {{id: "ambient/worker-memory", class: "conduit.resource/runtime-memory@1", slots: 67108864, bytes: 67108864}}
  need = {{id: "ambient/face-memory", class: "conduit.resource/runtime-memory@1", slots: {workspace}, bytes: {workspace}}}
  need = {{id: "ambient/face", class: "conduit.resource/face-slot@1", slots: 1, bytes: 1}}
  limits = {{static_memory_bytes: 16777216, heap_arena_bytes: 134217728, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}}
}}"#
    );
    let source = if matches!(
        projection,
        conduit_std_offers::AmbientFaceProjection::Wallpaper
    ) {
        source.replace(
            &format!(r#"timezone: "America/Los_Angeles", weather_endpoint: "{endpoint}", "#),
            "",
        )
    } else {
        source
    };
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/ambient-home".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    (checked, image)
}

#[test]
fn checked_ambient_image_selects_the_three_runtime_implementations() {
    let (_, image) = checked_image("127.0.0.1:12345".parse().unwrap());
    for id in [
        TIME_EVERY_IMPLEMENTATION,
        AMBIENT_HOME_SOURCE_IMPLEMENTATION,
        AMBIENT_HOME_FACE_IMPLEMENTATION,
    ] {
        assert!(image
            .manifest
            .implementations
            .iter()
            .any(|implementation| implementation == id));
    }
}

fn initialized_host(
    checked: &CheckedHostConfiguration,
    image: &HostImage,
    endpoint: std::net::SocketAddr,
) -> crate::StdHost {
    use conduit_core::{AuthorityGrant, OfferGeneration};
    let source = super::CheckedAmbientHomeSource::from_configuration(checked).unwrap();
    let clock_only =
        source.implementation == conduit_std_offers::AMBIENT_CLOCK_SOURCE_IMPLEMENTATION;
    let config = crate::StdHostConfig {
        host_id: "fixture/ambient-image".into(),
        boot_id: "boot/ambient-image".into(),
        offer_generation: OfferGeneration(1),
    };
    let offer = match source.implementation {
        conduit_std_offers::AMBIENT_CLOCK_SOURCE_IMPLEMENTATION => {
            conduit_std_offers::ambient_clock_source_offer()
        }
        conduit_std_offers::AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION => {
            conduit_std_offers::ambient_wallpaper_source_offer()
        }
        _ => conduit_std_offers::ambient_home_source_offer(),
    }
    .unwrap();
    let standalone_wallpaper =
        source.implementation == conduit_std_offers::AMBIENT_WALLPAPER_SOURCE_IMPLEMENTATION;
    assert_eq!(
        source.weather_endpoint,
        (!standalone_wallpaper).then_some(endpoint)
    );
    let requirement = &offer.authority_requirements[0];
    let grant = AuthorityGrant {
        grant_id: "grant/fixture-ambient-image".into(),
        contract_id: requirement.contract_id.clone(),
        host_call_contract_id: requirement.host_call_contract_id.clone(),
        subject_kind: offer.kind_id,
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
        capability_id: offer.capability_id,
    };
    // The fixture owns the exact loopback endpoint, checked pools and grant.
    unsafe {
        crate::StdHost::from_selected_ambient_home_image(
            config,
            image,
            &conduit_workspace_make::catalog(),
            checked,
            grant,
            || {
                assert!(
                    !clock_only,
                    "standalone clock must never acquire photographs"
                );
                Ok(Some(super::fixture_deck()))
            },
        )
    }
    .unwrap()
}

#[test]
fn original_ambient_image_initializes_the_exact_worker_and_face_offers() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    let (checked, image) = checked_image(endpoint);
    let host = initialized_host(&checked, &image, endpoint);
    assert!(
        host.native_source_worker_inspection()
            .unwrap()
            .idle_backing_is_reserved
    );
    assert_eq!(
        host.native_source_planning_grant().unwrap().host_id,
        host.advertisement().host_id,
    );
    assert!(host.advertisement().capabilities.iter().any(|offer| {
        offer.implementation.implementation_id.as_str() == AMBIENT_HOME_FACE_IMPLEMENTATION
    }));
}

fn ambient_plan(
    host: &crate::StdHost,
    projection: conduit_std_offers::AmbientFaceProjection,
) -> (conduit_body::Wake, conduit_body::BodyPlan) {
    use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };
    use std::collections::BTreeMap;
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_ambient_display_types(&mut startup).unwrap();
    conduit_semantic_catalog::install_ambient_home_observation_kind(&mut startup, &mut profiles)
        .unwrap();
    conduit_semantic_catalog::install_ambient_home_face_kind(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_ambient_clock_kind(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_ambient_clock_face_kind(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_resident_wallpaper_kinds(&mut startup, &mut profiles)
        .unwrap();
    let (source, entry) = match projection {
        conduit_std_offers::AmbientFaceProjection::Home => (
            include_str!("../../../../../plots/ambient-home/main.conduit"),
            "ambient-resident",
        ),
        conduit_std_offers::AmbientFaceProjection::Clock => (
            include_str!("../../../../../plots/resident-ambient-clock-weather/main.conduit"),
            "resident-ambient-clock-weather",
        ),
        conduit_std_offers::AmbientFaceProjection::Wallpaper => (
            include_str!("../../../../../plots/resident-ambient-wallpaper/main.conduit"),
            "resident-ambient-wallpaper",
        ),
    };
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, entry, &profiles).unwrap();
    let advertisements = std::slice::from_ref(host.advertisement());
    let placements =
        conduit_planner::default_expanded_placements(&authoring.expanded, advertisements).unwrap();
    let connection_limits = super::ambient_home_connection_limits(&authoring.expanded);
    let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
        &authoring,
        advertisements,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: std::slice::from_ref(host.native_source_planning_grant().unwrap()),
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connection_limits,
        &BTreeMap::new(),
    )
    .unwrap();
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "sign/home-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "sign/home-woke".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: resident,
            plan,
        }],
    )
    .unwrap();
    (wake, plan)
}

struct Capture {
    control: crate::RunControl,
    current: Option<crate::body_execution::CommittedFaceContribution>,
    repeated: bool,
}

impl crate::body_execution::BodyFaceContributionObserver for Capture {
    fn contribute(
        &mut self,
        contribution: crate::body_execution::CommittedFaceContribution,
    ) -> Result<(), crate::body_execution::ReturnedFaceContribution> {
        let previous = self.current.replace(contribution);
        if let Some(previous) = previous {
            assert!(previous.request() < self.current.as_ref().unwrap().request());
            self.repeated = true;
            self.control
                .request_stop(crate::RunControlRequestId::new("fixture/home-complete").unwrap())
                .unwrap();
        }
        Ok(())
    }
}

#[test]
fn funded_ambient_play_publishes_worker_observation_then_retires_its_face() {
    prove_funded_ambient_projection(conduit_std_offers::AmbientFaceProjection::Home);
}

#[test]
fn funded_standalone_clock_repeats_without_preparing_photographs() {
    prove_funded_ambient_projection(conduit_std_offers::AmbientFaceProjection::Clock);
}

#[test]
fn funded_standalone_wallpaper_presents_the_prepared_deck_without_weather() {
    prove_funded_ambient_projection(conduit_std_offers::AmbientFaceProjection::Wallpaper);
}

fn prove_funded_ambient_projection(projection: conduit_std_offers::AmbientFaceProjection) {
    use crate::{body_execution::BodyRunRequest, RunControl, ThreadTimer};
    let weather = (!matches!(
        projection,
        conduit_std_offers::AmbientFaceProjection::Wallpaper
    ))
    .then(super::weather_fixture_after_start);
    let endpoint = weather
        .as_ref()
        .map(|(endpoint, _, _)| *endpoint)
        .unwrap_or_else(|| "127.0.0.1:1".parse().unwrap());
    let (checked, image) = checked_image_for(endpoint, projection);
    let mut host = initialized_host(&checked, &image, endpoint);
    let (wake, plan) = ambient_plan(&host, projection);
    let control = RunControl::default();
    let mut observer = Capture {
        control: control.clone(),
        current: None,
        repeated: false,
    };
    let report = std::thread::scope(|scope| {
        let (done, cancellation) = std::sync::mpsc::sync_channel::<()>(1);
        let stop = control.clone();
        scope.spawn(move || {
            if cancellation
                .recv_timeout(std::time::Duration::from_secs(12))
                .is_err()
            {
                let _ = stop.request_stop(
                    crate::RunControlRequestId::new("fixture/home-deadline").unwrap(),
                );
            }
        });
        let report = host.run_selected_image_body_plan_with_face_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &mut Vec::new(),
            &mut ThreadTimer,
            &mut observer,
            |_, _| {
                if let Some((_, _, start)) = &weather {
                    start.send(()).map_err(|error| error.to_string())?;
                }
                Ok(())
            },
        );
        let _ = done.send(());
        report
    })
    .unwrap();
    assert!(
        report.report().failure.is_none(),
        "{:?}",
        report.report().failure
    );
    assert!(
        observer.repeated,
        "resident Play committed only its initial Face"
    );
    let face = observer
        .current
        .expect("funded Body Play committed the worker observation");
    assert_eq!(face.scope().body_play(), &report.report().play);
    assert!(face.scope().is_retired());
    let fragment = face.fragment();
    if let Some((_, weather, _)) = weather {
        assert!(fragment.properties.iter().any(|property| {
            property.name == "timezone"
                && property.value
                    == conduit_face::FacePropertyValue::Text("America/Los_Angeles".into())
        }));
        assert!(fragment
            .properties
            .iter()
            .any(|property| property.name == "local-date-time"));
        weather.join().unwrap();
        assert!(fragment
            .properties
            .iter()
            .any(|property| property.name == "availability"
                && property.value == conduit_face::FacePropertyValue::Text("fresh".into())));
    } else {
        assert!(fragment.subjects.iter().any(|subject| {
            subject.role == conduit_face::FaceRole::Semantic(conduit_core::kind_id("media/image"))
        }));
        assert!(fragment
            .properties
            .iter()
            .any(|property| property.name == "next-rotation-unix-ms"));
        assert!(!fragment
            .properties
            .iter()
            .any(|property| property.name == "local-date-time"));
    }
}
