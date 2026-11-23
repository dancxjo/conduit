use super::*;
use crate::{
    ambient_clock::PreparedAmbientClockWire,
    ambient_wallpaper::{
        acquisition::{
            prepare_deck, PreparationAdmission, MAXIMUM_RETAINED_BYTES, PREPARATION_BYTES,
        },
        MAXIMUM_APOD_RESPONSE_BYTES,
    },
};
use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder};

fn fixture_deck() -> PreparedDeck {
    let admission = PreparationAdmission::admit(
        chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        MAXIMUM_RETAINED_BYTES,
        PREPARATION_BYTES,
    )
    .unwrap();
    let mut number = 0;
    prepare_deck(admission, |requested, maximum| {
        if maximum == MAXIMUM_APOD_RESPONSE_BYTES {
            return Ok(serde_json::to_vec(&serde_json::json!({
                "date": requested.rsplit('=').next().unwrap(),
                "title": "Fixture photo",
                "media_type": "image",
                "hdurl": "https://apod.nasa.gov/fixture.png"
            }))
            .unwrap());
        }
        number += 1;
        let mut pixels = Vec::new();
        PngEncoder::new(&mut pixels)
            .write_image(&[number, 2, 3, 255], 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        Ok(pixels)
    })
    .unwrap()
}

#[test]
fn prepared_home_rotates_from_the_clock_observation_without_allocations() {
    let mut home = PreparedAmbientHomeObservation::prepare(
        "America/Los_Angeles",
        "127.0.0.1:80".parse().unwrap(),
        fixture_deck(),
    )
    .unwrap();
    let mut clock =
        PreparedAmbientClockWire::prepare("America/Los_Angeles", "Weather unavailable").unwrap();
    for (now, selected) in [(100_000, 100_000), (399_999, 100_000), (400_000, 400_000)] {
        let sample = clock.encode(now).unwrap();
        home.clock_bytes[..sample.len()].copy_from_slice(sample);
        let probe = crate::allocation_probe::begin();
        let encoded = home.compose(sample.len()).unwrap();
        assert_eq!(probe.finish_bytes(), 0);
        let observation = canonical(encoded).unwrap();
        let selection = observation
            .record_field("wallpaper")
            .unwrap()
            .unwrap()
            .variant_payload("available")
            .unwrap()
            .unwrap();
        let observed = selection
            .record_field("selected_at_unix_ms")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap();
        assert_eq!(u64::from_le_bytes(observed.try_into().unwrap()), selected);
    }
}

#[test]
fn home_keeps_the_clock_current_without_a_photo_deck() {
    let mut home = PreparedAmbientHomeObservation::prepare_with_optional_deck(
        "America/Los_Angeles",
        "127.0.0.1:80".parse().unwrap(),
        None,
    )
    .unwrap();
    assert!(home.photos().is_none());
    let mut clock =
        PreparedAmbientClockWire::prepare("America/Los_Angeles", "Weather unavailable").unwrap();
    for now in [100_000_u64, 400_000] {
        let sample = clock.encode(now).unwrap();
        home.clock_bytes[..sample.len()].copy_from_slice(sample);
        let probe = crate::allocation_probe::begin();
        let encoded = home.compose(sample.len()).unwrap();
        assert_eq!(probe.finish_bytes(), 0);
        let observation = canonical(encoded).unwrap();
        let clock = observation.record_field("clock_weather").unwrap().unwrap();
        let timestamp = clock.record_field("observed_at_unix_ms").unwrap().unwrap();
        assert_eq!(
            timestamp.primitive_bytes("value/u64").unwrap(),
            now.to_le_bytes()
        );
        let wallpaper = observation.record_field("wallpaper").unwrap().unwrap();
        assert!(wallpaper.variant_payload("unavailable").unwrap().is_some());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn ambient_home_worker_becomes_an_installed_offer_only_with_its_exact_grant_and_backing() {
    use crate::{NativeSourceDefinition, NativeWorkerBudget, StdHost, StdHostConfig};
    use conduit_core::*;
    use conduit_std_offers::*;
    use std::time::Duration;

    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: "test/ambient-host".into(),
        boot_id: "test/ambient-boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let offer = ambient_home_source_offer().unwrap();
    let kind = conduit_semantic_catalog::ambient_home_observation_kind().unwrap();
    let budget = NativeWorkerBudget {
        requested_stack_bytes: 2 * 1024 * 1024,
        maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 64 * 1024,
        capture_timeout: Duration::from_secs(3),
        cancellation_timeout: Duration::from_millis(250),
    };
    let grant = AuthorityGrant {
        grant_id: "test/ambient-grant".into(),
        contract_id: AMBIENT_HOME_SOURCE_AUTHORITY.into(),
        host_call_contract_id: AMBIENT_HOME_SOURCE_CALL.into(),
        subject_kind: kind.kind_id.clone(),
        host_id: host.advertisement().host_id.clone(),
        boot_id: host.advertisement().boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    let output_bytes = HOME_OBSERVATION_BYTES as u32;
    let input_bytes = conduit_time::TICK_ENCODED_LEN;
    let (endpoint, weather_server) = weather_fixture();
    let provider =
        PreparedAmbientHomeObservation::prepare("America/Los_Angeles", endpoint, fixture_deck())
            .unwrap();
    let backing = budget.backing_bytes(input_bytes, output_bytes).unwrap();
    let mut definition = NativeSourceDefinition {
        kind,
        grant,
        base: BaseProviderEntry {
            base_id: "test/ambient-base".into(),
            provider_instance_id: "test/ambient-session".into(),
            provider_generation: 1,
            implementation_id: AMBIENT_HOME_SOURCE_IMPLEMENTATION.into(),
            mechanism_family: conduit_semantic_catalog::AMBIENT_HOME_OBSERVE_KIND.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer.clone()],
            resources: vec![
                resource_offer("test/ambient/source", AMBIENT_HOME_SOURCE_RESOURCE, 1),
                resource_offer(
                    "test/ambient/backing",
                    RUNTIME_MEMORY_RESOURCE_CLASS,
                    backing,
                ),
            ],
        },
        resource_generation: ResourceGenerationId("test/ambient/1".into()),
        configuration: vec![ConfigurationEntry {
            key: "timezone".into(),
            value: ConfigurationValue::Text("America/Los_Angeles".into()),
        }],
        input_bytes,
        output_bytes,
        resource_bytes: u64::from(output_bytes),
    };
    let correct_boot = definition.grant.boot_id.clone();
    definition.grant.boot_id = "test/stale-boot".into();
    // The exact prepared fixture owns fixed local photo bytes and can only
    // contact the declared loopback peer. No read is made by installation.
    let (mut definition, provider, _) =
        unsafe { host.install_native_source_worker(definition, budget, Box::new(provider)) }
            .expect_err("a stale grant must refuse before worker startup");
    assert!(!host.advertisement().capabilities.contains(&offer));
    assert!(host.native_source_planning_grant().is_none());
    definition.grant.boot_id = correct_boot;
    let grant = definition.grant.clone();
    unsafe { host.install_native_source_worker(definition, budget, provider) }
        .unwrap_or_else(|(_, _, reason)| panic!("{reason}"));
    assert!(host.advertisement().capabilities.contains(&offer));
    assert_eq!(host.native_source_planning_grant(), Some(&grant));
    let plan = plan_fixture(&host, grant);
    let placement = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|gear| gear.capability_id == offer.capability_id)
        .unwrap();
    assert_eq!(placement.host_calls, offer.host_calls);
    assert_eq!(placement.authority[0].boot_id, host.advertisement().boot_id);
    let mut capture = HomeCapture {
        readings: 0,
        faces: conduit_semantic_catalog::PreparedAmbientFaces::prepare().unwrap(),
    };
    host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[crate::ExternalForeInput {
            front_port_id: "pulse".into(),
            track: ConnectionTrack::Payload,
            bytes: conduit_time::encode_tick(0).to_vec(),
        }],
        &mut capture,
        &mut Vec::new(),
        &mut crate::ThreadTimer,
    )
    .unwrap();
    weather_server.join().unwrap();
    assert_eq!(capture.readings, 1);
}

#[cfg(target_os = "linux")]
fn plan_fixture(host: &crate::StdHost, grant: conduit_core::AuthorityGrant) -> conduit_core::Plan {
    use conduit_core::*;
    use conduit_planner::{
        default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
        ForeBoundaryKey, PlanningOptions,
    };
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
        ProfileCatalog, StartupCatalog,
    };
    use conduit_semantic_catalog as catalog;
    use std::collections::BTreeMap;

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    catalog::install_ambient_display_types(&mut startup).unwrap();
    catalog::install_ambient_home_observation_kind(&mut startup, &mut profile).unwrap();
    startup
        .insert_value_kind_alias("Tick", kind_id(conduit_time::TICK_VALUE_KIND))
        .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(
            r#"
plot home (
    >> pulse: Tick...
    current: AmbientHomeObservation... <= 8192B >>
) {
    source: ambient/resident-observation("America/Los_Angeles")
    pulse >> source.tick
    source.observation >> current
}
"#,
        ),
        &startup,
    )
    .unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "home", &profile).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (
            PortDirection::Input,
            "pulse",
            conduit_time::TICK_ENCODED_LEN,
        ),
        (
            PortDirection::Output,
            "current",
            HOME_OBSERVATION_BYTES as u32,
        ),
    ]
    .into_iter()
    .map(|(direction, name, byte_capacity)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity,
            },
        )
    })
    .collect();
    plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: HOME_OBSERVATION_BYTES as u32,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap()
}

#[cfg(target_os = "linux")]
struct HomeCapture {
    readings: usize,
    faces: conduit_semantic_catalog::PreparedAmbientFaces,
}

#[cfg(target_os = "linux")]
impl crate::ExternalForeOutputAdapter for HomeCapture {
    fn deliver(&mut self, _: crate::ExternalForeDelivery) -> Result<(), String> {
        Err("the Home fixture requires committed delivery".into())
    }
    fn deliver_committed(
        &mut self,
        witness: crate::CommittedForeValue<'_>,
        _: crate::ExternalForeDelivery,
    ) -> Result<(), String> {
        assert_eq!(witness.scope().fore.front_port_id.as_str(), "current");
        let home = canonical(witness.bytes()).unwrap();
        let clock = home.record_field("clock_weather").unwrap().unwrap();
        assert_eq!(
            clock
                .record_field("weather")
                .unwrap()
                .unwrap()
                .variant_tag()
                .unwrap(),
            "fresh"
        );
        assert_eq!(
            home.record_field("wallpaper")
                .unwrap()
                .unwrap()
                .variant_tag()
                .unwrap(),
            "available"
        );
        let scope = witness.scope();
        let contribution = self
            .faces
            .home_canonical_observation(
                conduit_face::FaceContributionBasis {
                    checked_plot_id: scope.fragment.checked_plot_id.clone(),
                    plan_id: scope.fragment.plan_id.clone(),
                    active_play_id: scope.play.active_play_id.clone(),
                    required_interaction_context: None,
                },
                witness.bytes(),
            )
            .unwrap();
        assert_eq!(contribution.basis.active_play_id, scope.play.active_play_id);
        assert!(contribution.properties.iter().any(|property| {
            matches!(&property.value, conduit_face::FacePropertyValue::Content(_))
        }));
        assert!(contribution
            .text
            .iter()
            .any(|text| text.text.contains("Seattle")));
        verify_native_picture(contribution, scope);
        self.readings += 1;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn verify_native_picture(
    fragment: conduit_face::FaceFragment,
    scope: &crate::CommittedForeScope<'_>,
) {
    use conduit_face::{picture_frame_expression, Face, FaceBasis};
    use conduitos::native_face_scene::NativeFaceScene;

    let face = Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(scope.fragment.source_document_id.clone()),
            checked_plot_id: Some(fragment.basis.checked_plot_id),
            expanded_plot_id: Some(scope.fragment.expanded_plot_id.clone()),
            plan_id: Some(fragment.basis.plan_id),
            active_play_id: Some(fragment.basis.active_play_id),
            sign_ids: Vec::new(),
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .and_then(|face| face.with_composition(fragment.composition))
    .unwrap();
    let expression = picture_frame_expression().unwrap();
    for (width, height) in [(1024, 600), (640, 480)] {
        let mut picture = NativeFaceScene::prepare_picture_frame(
            face.clone(),
            width,
            height,
            true,
            &expression,
            Some(conduit_core::Unit::Celsius),
        )
        .unwrap();
        let frame = picture.frame().unwrap();
        let commands = frame.scene.commands();
        assert!(commands.iter().any(|command| command.image_key().is_some()));
        assert!(commands
            .iter()
            .any(|command| command.payload().contains("Clear")));
        assert!(commands.iter().all(|command| {
            command.bounds.x >= 0
                && command.bounds.y >= 0
                && command.bounds.x as u16 + command.bounds.width <= width
                && command.bounds.y as u16 + command.bounds.height <= height
        }));
        for _ in 1..picture.pages() {
            picture.turn_page(true);
        }
        assert!(picture.page() > 0);
        picture.turn_page(false);
        assert_eq!(picture.frame().unwrap().page, picture.pages() - 2);
    }
}

#[cfg(target_os = "linux")]
fn weather_fixture() -> (SocketAddr, std::thread::JoinHandle<()>) {
    let (endpoint, worker, start) = weather_fixture_after_start();
    start.send(()).unwrap();
    (endpoint, worker)
}

#[cfg(target_os = "linux")]
fn weather_fixture_after_start() -> (
    SocketAddr,
    std::thread::JoinHandle<()>,
    std::sync::mpsc::SyncSender<()>,
) {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = listener.local_addr().unwrap();
    let (start, begun) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        if begun.recv().is_err() {
            return;
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "weather request was never submitted"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("weather fixture accept: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
            assert!(request.len() < 1024);
        }
        assert!(request.starts_with(b"GET /v1/forecast?"));
        let observed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 1;
        let body = format!(
            r#"{{"current_units":{{"time":"unixtime","temperature_2m":"°C","weather_code":"wmo code"}},"current":{{"time":{observed},"temperature_2m":12.5,"weather_code":0}}}}"#
        );
        write!(
            stream,
            "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    (endpoint, worker, start)
}

#[test]
fn ambient_home_source_connects_the_typed_timer_observation_and_face() {
    use conduit_plot::{
        check_syntax_document, expand_canonical_plot, parse_syntax_document, ProfileCatalog,
        StartupCatalog,
    };
    use conduit_semantic_catalog as catalog;

    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    catalog::install_ambient_display_types(&mut startup).unwrap();
    catalog::install_ambient_home_observation_kind(&mut startup, &mut profile).unwrap();
    catalog::install_ambient_home_face_kind(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../../plots/ambient-home/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "ambient-resident", &profile).unwrap();

    let mut kinds: Vec<_> = expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.as_str())
        .collect();
    kinds.sort_unstable();
    assert_eq!(
        kinds,
        [
            "ambient/resident-observation",
            "face/resident-ambient-home",
            "time/every"
        ]
    );
    let gear_id = |kind: &str| {
        &expanded
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == kind)
            .unwrap()
            .gear_id
    };
    let cadence = gear_id("time/every");
    let source = gear_id("ambient/resident-observation");
    let face = gear_id("face/resident-ambient-home");
    assert_eq!(expanded.connections.len(), 2);
    assert!(expanded.connections.iter().any(|cord| {
        &cord.source_gear_id == cadence
            && &cord.sink_gear_id == source
            && cord.source_port_id.as_str() == "tick"
            && cord.sink_port_id.as_str() == "tick"
            && cord.value_kind.as_str() == conduit_time::TICK_VALUE_KIND
    }));
    assert!(expanded.connections.iter().any(|cord| {
        &cord.source_gear_id == source
            && &cord.sink_gear_id == face
            && cord.source_port_id.as_str() == "observation"
            && cord.sink_port_id.as_str() == "observation"
    }));
}

#[cfg(target_os = "linux")]
#[path = "tests/funded_image.rs"]
mod funded_image;
