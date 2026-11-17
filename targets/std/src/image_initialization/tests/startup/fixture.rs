//! Actual current checked IMAGE and original retained Body lifecycle fixture.
use super::*;
use std::collections::BTreeMap;
pub(super) fn source(heap: usize) -> String {
    super::super::SOURCE.replace("heap_arena_bytes: 67108864",&format!("heap_arena_bytes: {heap}"))
        .replace("  limits =", &format!(r#"  base = {{kind: "audio/speaker", implementation: "std/kernel-audio-play-alsa-hw@1", initialization: {{executable_sha256: "{}", card_id: "Fixture", device: 0, base_identity: "fixture/speaker"}}}}
  capability = {{kind: "sound/startup-chime", pinned_implementation: "std/kernel-startup-chime@1"}}
  capability = {{kind: "body/wake", pinned_implementation: "std/kernel-body-wake@1"}}
  need = {{id: "std/audio/alsa/fixture/speaker/card-Fixture/device-0", class: "conduit.resource/audio-playback-alsa-hw@1", slots: 1, bytes: 1}}
  limits ="#, "ab".repeat(32)))
}
pub(super) fn equipment(
    behavior: FakePlaybackBehavior,
    heap: usize,
) -> (
    crate::StdHostConfig,
    HostedPlaybackSelection,
    conduit_host_make::CheckedHostConfiguration,
    HostImage,
) {
    let config = crate::StdHostConfig {
        host_id: "host/startup-fixture".into(),
        boot_id: "boot/startup-fixture".into(),
        offer_generation: OfferGeneration(1),
    };
    let selected = HostedPlaybackSelection::deterministic_fake(
        AlsaPlaybackObservation {
            card_index: 999,
            card_id: "Fixture".into(),
            card_name: "Fixture".into(),
            device: 0,
            device_name: "PCM".into(),
            base_identity: "fixture/speaker".into(),
        },
        config.boot_id.clone(),
        config.offer_generation,
        behavior,
    )
    .with_executable_sha256("ab".repeat(32))
    .unwrap();
    let source = source(heap);
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    (config, selected, checked, super::super::image(&source))
}
pub(super) fn fixture(behavior: FakePlaybackBehavior) -> (StdHost, BodyLifecycleSession) {
    let (config, selected, checked, image) = equipment(behavior, 67108864);
    let host = StdHost::from_selected_timer_and_playback_image(
        config,
        &image,
        &conduit_workspace_make::catalog(),
        &checked,
        &selected,
    )
    .unwrap();
    session_for_host(host)
}
pub(super) fn session_for_host(host: StdHost) -> (StdHost, BodyLifecycleSession) {
    let config = crate::StdHostConfig {
        host_id: host.advertisement().host_id.clone(),
        boot_id: host.advertisement().boot_id.clone(),
        offer_generation: host.advertisement().offer_generation,
    };
    let plot = expanded_cue();
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&plot.expanded, &hosts).unwrap();
    let grant = host
        .startup_chime_authority_grant("grant/fixture-explicit")
        .unwrap();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::BOOL_ENCODED_LEN as u32,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &BTreeMap::new(),
    )
    .unwrap();
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let body = Body::born_with_plots(
        BodyWorkset::one(resident.clone()).unwrap(),
        1,
        "fixture/birth".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut biography = BodyBiographyEvidence::born(
        body.clone(),
        membership.clone(),
        "Existing fixture Body".into(),
    )
    .unwrap();
    let part = PartId::bind(&body.body_id, config.host_id.as_str(), 0).unwrap();
    let proof = MembershipProofId::bind("fixture/explicit-member").unwrap();
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "fixture/admit".into(),
        )
        .unwrap();
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: config.host_id.clone(),
                boot_id: config.boot_id.clone(),
                offer_generation: config.offer_generation,
                proof_id: proof,
                sequence: 0,
            },
            "fixture/present".into(),
        )
        .unwrap();
    let events = membership
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| (e.change_id.clone(), i as u64 + 2))
        .collect::<Vec<_>>();
    biography
        .append_membership_events(membership, &events)
        .unwrap();
    let mut session =
        BodyLifecycleSession::open_admitted(biography, &config.host_id, &config.boot_id).unwrap();
    session
        .propose(
            vec![BodyPlotPlan {
                plot: resident,
                plan,
            }],
            &config.host_id,
            &config.boot_id,
        )
        .unwrap();
    (host, session)
}

pub(super) fn expanded_cue() -> conduit_plot::ExpandedAuthoringPlot {
    let (mut startup, mut profiles) = (
        conduit_plot::StartupCatalog::new(),
        conduit_plot::ProfileCatalog::new(),
    );
    conduit_semantic_catalog::install_body_startup_catalogs(&mut startup, &mut profiles).unwrap();
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../plots/startup-chime/main.conduit"
    ));
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    conduit_plot::expand_canonical_plot_for_authoring(&checked, "startup_chime", &profiles).unwrap()
}
