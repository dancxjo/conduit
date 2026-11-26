//! Selected Linux IMAGE/closed-Source CPU proof. Reported producer identities
//! are fixture data, not a live sensor, product CLI or release receipt.
use super::tests::{facts, json, publish};
use crate::environment_recording_history::snapshot_fixture::Directory;
use crate::{body_execution::*, StdHost, StdHostConfig, ThreadTimer};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot, Wake};
use conduit_core::*;
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, CheckedHostConfiguration, HostImage, MakeCatalog,
};
use conduit_std_offers::*;
use std::{fs, time::Duration};

fn equipment_source() -> String {
    let workspace = conduit_semantic_catalog::quantity_observation_face_workspace_bytes().unwrap();
    let maximum = conduit_data::resolved_quantity_observation_snapshot_byte_bound().unwrap();
    let source = format!(
        r#"host environment-recording-installed-fixture {{
 schema = 1
 target = {{architecture: "x86_64", machine: "computer", os: "linux"}}
 base = {{kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}}
 base = {{kind: "{ENVIRONMENT_RECORDING_HISTORY_BASE}", implementation: "{ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION}", initialization: {{source_pool: "history/source", memory_pool: "history/memory", capture_access: "operator-admitted", directory: "", reference: "", producer_host: "host/reported", producer_boot: "boot/reported", producer_plan: "plan/reported", producer_play: "play/reported", temperature_source: "source/t", temperature_channel: "channel/t", temperature_name: "Temperature", humidity_source: "source/h", humidity_channel: "channel/h", humidity_name: "Relative humidity", tail: 2}}}}
 base = {{kind: "{QUANTITY_OBSERVATION_FACE_BASE}", implementation: "{QUANTITY_OBSERVATION_FACE_DRIVER}", initialization: {{face_pool: "history/face", memory_pool: "history/face-memory"}}}}
 capability = {{kind: "time/every", pinned_implementation: "{TIME_EVERY_IMPLEMENTATION}"}}
 capability = {{kind: "data/environment-recording-history", pinned_implementation: "{ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION}"}}
 capability = {{kind: "face/quantity-observation-series", pinned_implementation: "{QUANTITY_OBSERVATION_FACE_IMPLEMENTATION}"}}
 need = {{id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}}
 need = {{id: "history/source", class: "{ENVIRONMENT_RECORDING_HISTORY_RESOURCE}", slots: 1, bytes: {maximum}}}
 need = {{id: "history/memory", class: "conduit.resource/runtime-memory@1", slots: 67108864, bytes: 67108864}}
 need = {{id: "history/face", class: "conduit.resource/face-slot@1", slots: 1, bytes: 1}}
 need = {{id: "history/face-memory", class: "conduit.resource/runtime-memory@1", slots: {workspace}, bytes: {workspace}}}
 limits = {{static_memory_bytes: 16777216, heap_arena_bytes: 268435456, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}}
}}"#
    );
    source
}
fn equipment() -> (CheckedHostConfiguration, HostImage, MakeCatalog) {
    equipment_from(&equipment_source()).unwrap()
}
fn equipment_from(
    source: &str,
) -> Result<(CheckedHostConfiguration, HostImage, MakeCatalog), String> {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(source).map_err(|why| format!("{why:?}"))?,
        &catalog,
        &packages,
    )
    .map_err(|why| format!("{why:?}"))?;
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/closed-environment-recording".into(),
            toolchain_available: true,
        },
    )
    .map_err(|why| format!("{why:?}"))?;
    Ok((checked, image, catalog))
}
fn config() -> StdHostConfig {
    StdHostConfig {
        host_id: "host/capture-fixture".into(),
        boot_id: "boot/current-fixture".into(),
        offer_generation: OfferGeneration(1),
    }
}
fn source_grant(config: &StdHostConfig) -> AuthorityGrant {
    // Separate operator fixture authorization; the storage read grants do not
    // manufacture this Source-use grant or current native Base correspondence.
    AuthorityGrant {
        grant_id: "grant/captured-history-source-use".into(),
        contract_id: ENVIRONMENT_RECORDING_HISTORY_AUTHORITY.into(),
        host_call_contract_id: ENVIRONMENT_RECORDING_HISTORY_CALL.into(),
        subject_kind: conduit_semantic_catalog::ENVIRONMENT_RECORDING_HISTORY_KIND.into(),
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
        capability_id: ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION.into(),
    }
}
fn body_plan(host: &StdHost) -> (Wake, BodyPlan) {
    use conduit_plot::*;
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_environment_recording_history_kind(
        &mut startup,
        &mut profile,
    )
    .unwrap();
    conduit_semantic_catalog::install_quantity_observation_face_kind(&mut startup, &mut profile)
        .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../plots/environment-recording-history/main.conduit"
        )),
        &startup,
    )
    .unwrap();
    // Ordinary closed-root expansion, never the unbound authoring entrance.
    let expanded =
        expand_canonical_plot(&checked, "environment-recording-history", &profile).unwrap();
    let hosts = std::slice::from_ref(host.advertisement());
    let placements = conduit_planner::default_expanded_placements(&expanded, hosts).unwrap();
    let queues =
        crate::environment_recording_history::environment_recording_history_connection_limits(
            &expanded,
        )
        .unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &expanded,
        hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &Default::default(),
            line_candidates: &Default::default(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_time::TICK_ENCODED_LEN,
            authority_grants: std::slice::from_ref(host.native_source_planning_grant().unwrap()),
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &queues,
    )
    .unwrap();
    assert!(plan
        .fragments
        .iter()
        .all(|fragment| fragment.fore_ports.is_empty()));
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "sign/captured-history-born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "sign/captured-history-wake".into()).unwrap().1;
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
    current: Option<CommittedFaceContribution>,
    accepted: usize,
    stop: Option<crate::RunControlRequestId>,
}
impl BodyFaceContributionObserver for Capture {
    fn contribute(
        &mut self,
        contribution: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        if let Some(previous) = &self.current {
            assert!(contribution.request() > previous.request());
            assert_eq!(
                contribution.scope().body_play(),
                previous.scope().body_play()
            );
        }
        self.accepted += 1;
        // Retain one current contribution; do not manufacture revisions or ACKs.
        drop(self.current.replace(contribution));
        if self.accepted == 2 {
            self.control
                .request_stop(self.stop.take().expect("one completed fixture stop"))
                .unwrap();
        }
        Ok(())
    }
}
#[test]
fn closed_selected_image_freezes_original_recording_and_commits_two_channels() {
    let root = Directory::new();
    let directory = root.open();
    let contract = conduit_dht11_observation::ObservationContract::prepare().unwrap();
    let bytes = json(&contract);
    publish(&directory, &bytes);
    let selected = facts(bytes.len(), false);
    let (checked, image, catalog) = equipment();
    let config = config();
    // SAFETY: fixture owns actual directory/current independent read selections
    // and separately authorizes Source use on the selected IMAGE Host/Boot.
    let mut host = unsafe {
        StdHost::from_selected_environment_recording_history_image(
            config.clone(),
            &image,
            &catalog,
            &checked,
            source_grant(&config),
            selected.capture(&directory),
        )
    }
    .unwrap();
    drop(directory);
    fs::remove_dir_all(&root.0).unwrap();
    assert!(
        host.native_source_worker_inspection()
            .unwrap()
            .idle_backing_is_reserved
    );
    let (wake, plan) = body_plan(&host);
    let control = crate::RunControl::default();
    let mut observer = Capture {
        control: control.clone(),
        current: None,
        accepted: 0,
        stop: Some(crate::RunControlRequestId::new("fixture/captured-history-complete").unwrap()),
    };
    let report = std::thread::scope(|scope| {
        let (done, finish) = std::sync::mpsc::sync_channel::<()>(1);
        let stop = control.clone();
        scope.spawn(move || {
            if finish.recv_timeout(Duration::from_secs(30)).is_err() {
                let _ = stop.request_stop(
                    crate::RunControlRequestId::new("fixture/captured-history-deadline").unwrap(),
                );
            }
        });
        // The actual admitted monotonic timer preserves the authored one-second
        // cadence. The watchdog bounds a failed test, never drives an action.
        let result = host.run_selected_image_body_plan_with_face_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &mut Vec::new(),
            &mut ThreadTimer,
            &mut observer,
            |_, _| Ok(()),
        );
        let _ = done.send(());
        result
    })
    .unwrap();
    assert!(
        report.report().failure.is_none(),
        "{:?}",
        report.report().failure
    );
    assert!(
        report.report().kernel_failure.is_none(),
        "{:?}",
        report.report().kernel_failure
    );
    assert!(
        report.report().cleanup_failure.is_none(),
        "{:?}",
        report.report().cleanup_failure
    );
    assert_eq!(observer.accepted, 2);
    let source = plan.plots[0].plan.fragments[0]
        .placements
        .iter()
        .find(|gear| {
            gear.kind_id.as_str() == conduit_semantic_catalog::ENVIRONMENT_RECORDING_HISTORY_KIND
        })
        .unwrap();
    let timer = plan.plots[0].plan.fragments[0]
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_time::TIME_EVERY_KIND)
        .unwrap();
    for placement in [source, timer] {
        let node = report.report().partitions[0]
            .node_for_placement(&placement.placement_id)
            .unwrap();
        assert!(
            report
                .report()
                .kernel_events
                .iter()
                .filter(|event| event.node == node
                    && event.kind == conduit_kernel::KernelEventKind::HostCallCompleted)
                .count()
                >= 2,
            "{} did not complete twice",
            placement.kind_id.as_str()
        );
    }
    let contribution = observer.current.as_ref().unwrap();
    assert_eq!(contribution.scope().body_play(), &report.report().play);
    assert_eq!(contribution.scope().host(), &config.host_id);
    assert_eq!(contribution.scope().boot(), &config.boot_id);
    assert!(contribution.scope().is_retired());
    let fragment = contribution.fragment();
    assert_eq!(
        fragment.basis.active_play_id,
        report.report().play.active_play_id
    );
    use conduit_face::FacePropertyValue as V;
    let properties = &fragment.properties;
    let values = |name: &str| {
        properties
            .iter()
            .filter(|p| p.name == name)
            .map(|p| p.value.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        values("producer-host"),
        vec![V::Identity("host/reported".into()); 2]
    );
    assert_eq!(
        values("producer-boot"),
        vec![V::Identity("boot/reported".into()); 2]
    );
    assert_eq!(
        values("producer-plan"),
        vec![V::Identity("plan/reported".into()); 2]
    );
    assert_eq!(
        values("producer-play"),
        vec![V::Identity("play/reported".into()); 2]
    );
    assert_eq!(
        values("sampled-at-boot-ms"),
        vec![V::Count(7), V::Count(101), V::Count(7), V::Count(101)]
    );
    assert_eq!(
        values("sequence"),
        vec![V::Count(4), V::Count(5), V::Count(4), V::Count(5)]
    );
    assert_eq!(values("missing-sequence-first"), vec![V::Count(2); 2]);
    assert_eq!(values("missing-sequence-last"), vec![V::Count(3); 2]);
    assert_eq!(values("samples-outside-view"), vec![V::Count(1); 2]);
    assert_eq!(
        values("series-unavailability"),
        vec![V::Text("checksum".into()); 2]
    );
    assert_eq!(
        values("series-available"),
        vec![V::Flag(false), V::Flag(true), V::Flag(false), V::Flag(true)]
    );
    let channels = [
        ["source/t", "Temperature", "channel/t", "Temperature"],
        [
            "source/h",
            "Relative humidity",
            "channel/h",
            "Relative humidity",
        ],
    ];
    for names in channels {
        assert!(fragment
            .subjects
            .iter()
            .any(|s| s.identity == names[0] && s.name == names[1]));
        assert!(fragment
            .subjects
            .iter()
            .any(|s| s.identity == names[2] && s.name == names[3]));
    }
    // Available values are the original exact quantity Forms, never copied onto
    // failed observations or converted through display units.
    let mut forms =
        conduit_dht11_observation::PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let canonical =
        conduit_core::validate_canonical_structured_value(forms.unpack(&[198, 0, 78, 2]).unwrap())
            .unwrap();
    for (source, field) in [("source/t", "temperature"), ("source/h", "humidity")] {
        let leaf = canonical.record_field(field).unwrap().unwrap();
        let expected = Quantity::decode(&leaf.value_node()[5..]).unwrap();
        let available = format!("{source}/sample/5");
        let failed = format!("{source}/sample/4");
        assert!(properties
            .iter()
            .any(|p| p.subject == available && matches!(&p.value, V::TypedValue { bytes, .. } if Quantity::decode(bytes) == Ok(expected))));
        assert!(!properties
            .iter()
            .any(|p| p.subject == failed && matches!(p.value, V::TypedValue { .. })));
    }
}

#[path = "admission_tests.rs"]
mod admission;
