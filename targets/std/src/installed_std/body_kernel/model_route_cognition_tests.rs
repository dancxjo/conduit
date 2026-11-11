//! Fenced fake evidence-dependent model proof, not autonomous memory retrieval.
use super::*;
use conduit_human::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[allow(dead_code)]
#[path = "../../../../../semantics/catalog/tests/common/experience_evidence.rs"]
mod evidence;

/// Keep the fake explicitly dependent on the typed evidence delivered to it.
pub(super) fn reply(input: &[u8]) -> Option<Vec<u8>> {
    let prompt: serde_json::Value = serde_json::from_slice(input).ok()?;
    let items = prompt["evidence"]["items"].as_array()?;
    assert!(items.iter().any(|item| item["origin"] == "observation"));
    let remembered = items.iter().find(|item| item["origin"] == "remembered");
    Some(if let Some(memory) = remembered {
        assert_eq!(memory["temporal_role"], "historical");
        assert!(memory["sources"].to_string().contains("episode/1"));
        b"I recall earlier evidence alongside the current observation.".to_vec()
    } else {
        b"I have only the current observation, with uncertainty.".to_vec()
    })
}

fn cognition_plan(host: &StdHost) -> Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_experience_evidence_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_experience_integration_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_experience_prompt_catalogs(&mut startup, &mut profile)
        .unwrap();
    install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    startup
        .insert_value_kind_alias(
            "PeteCognitionResponse",
            conduit_core::kind_id(GENERATED_RESULT_VALUE_KIND),
        )
        .unwrap();
    let source = include_str!("../../../../../plots/pete-cognition/main.conduit");
    let syntax = parse_syntax_document(source);
    assert_eq!(syntax.round_trip(), source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "pete-cognition", &profile).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        ("updates", PortDirection::Input),
        ("responses", PortDirection::Output),
    ]
    .into_iter()
    .map(|(name, direction)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: if direction == PortDirection::Input {
                    65_536
                } else {
                    16_384
                },
            },
        )
    })
    .collect();
    plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 16_384,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap()
}

fn cognition_inputs() -> Vec<ExternalForeInput> {
    let observed = evidence::observation("light/current");
    // An explicitly selected autobiographical record, supplied through a Cord.
    let recalled = evidence::with_sources(
        evidence::item(
            "light/recalled",
            ExperienceOrigin::Remembered,
            ExperienceTemporalRole::Historical,
            ExperienceAvailability::Present,
            7,
            "provenance/7",
            b"Earlier the hallway light was on",
        ),
        vec![ExperienceEvidenceSource::memory_record(evidence::identity("episode/1")).unwrap()],
    );
    [(0, observed), (1, recalled)]
        .into_iter()
        .map(|(revision, item)| {
            let operation = ExperienceIntegrationOperation::upsert(
                *item.availability(),
                *item.certainty(),
                item.content_kind().clone(),
                *item.domain(),
                item.encoded_content().clone(),
                item.identity().clone(),
                *item.origin(),
                item.provenance_revision().clone(),
                *item.revision(),
                item.sources().clone(),
                *item.temporal_role(),
            )
            .unwrap();
            ExternalForeInput {
                front_port_id: "updates".into(),
                track: ConnectionTrack::Payload,
                bytes: ExperienceIntegrationUpdate::new(
                    revision,
                    operation,
                    evidence::identity("snapshot/cognition-proof"),
                )
                .unwrap()
                .encode()
                .unwrap(),
            }
        })
        .collect()
}

#[test]
fn authored_cognition_and_clock_share_actual_body_wake_and_play_across_two_updates() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut host = StdHost::new_with_local_model(
        StdHostConfig {
            host_id: "host/cognition-body-proof".into(),
            boot_id: "boot/cognition-body-proof".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::reference(),
        Box::new(Fake {
            offer: offer(),
            calls: calls.clone(),
            lost: false,
        }),
    )
    .unwrap();
    let plans = [
        cognition_plan(&host),
        plan(
            &host,
            include_str!("../../../../../plots/clock/main.conduit"),
            "clock-demo",
        ),
    ];
    let mut body = Body::born(
        plans[0].source_document_id.clone(),
        plans[0].checked_plot_id.clone(),
        1,
        "sign/cognition-born".into(),
    )
    .unwrap();
    body = body
        .admit_plot(
            ResidentPlot::new(
                plans[1].source_document_id.clone(),
                plans[1].checked_plot_id.clone(),
            ),
            "sign/cognition-clock-admitted".into(),
        )
        .unwrap();
    let wake = body.wake(1, "sign/cognition-wake".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        plans
            .into_iter()
            .map(|plan| BodyPlotPlan {
                plot: ResidentPlot::new(
                    plan.source_document_id.clone(),
                    plan.checked_plot_id.clone(),
                ),
                plan,
            })
            .collect(),
    )
    .unwrap();
    let control = RunControl::default();
    let delivered = Arc::new(AtomicUsize::new(0));
    let mut capture = Capture {
        values: vec![],
        delivered: delivered.clone(),
    };
    let mut clock = Clock {
        now: 0,
        waits: 0,
        delivered,
        control: control.clone(),
    };
    let mut output = Vec::new();
    let report = host
        .run_body_plan_with_fore_to(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &cognition_inputs(),
            true,
            &mut capture,
            &mut output,
            &mut clock,
        )
        .unwrap();
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert_eq!(report.wake_at_start.body_id, wake.body_id);
    assert_eq!(report.wake_at_start.wake_id, wake.wake_id);
    assert!(report.play.validate_for(&plan));
    assert_eq!(report.partitions.len(), 2);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(capture.values.len(), 2);
    assert_ne!(capture.values[0].payload, capture.values[1].payload);
    assert_eq!(
        capture.values[0].payload,
        b"I have only the current observation, with uncertainty."
    );
    assert_eq!(
        capture.values[1].payload,
        b"I recall earlier evidence alongside the current observation."
    );
    assert!(capture
        .values
        .iter()
        .all(|value| value.provenance == ModelResultProvenance::ModelDerived));
    assert!(String::from_utf8(output).unwrap().contains("Clock tick"));
    assert!(clock.waits >= 3);
}
