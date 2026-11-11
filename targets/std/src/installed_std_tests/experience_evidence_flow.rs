#[allow(dead_code)] // Shared fixture functions serve several independent proof surfaces.
#[path = "../../../../semantics/catalog/tests/common/experience_evidence.rs"]
mod common;
use super::{host, RecordingTimer};
use crate::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, RunControl,
    RunControlRequestId,
};
use common::*;
use conduit_core::{
    BaseImplementationId, ConnectionTrack, ObservationKind, PortDirection, TerminalDisposition,
};
use conduit_human::*;
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use std::collections::BTreeMap;

const PROGRAM: &str = "plot evidence-flow (\n >> requests: ExperienceEvidenceRequest...|\n contexts: ExperienceEvidenceContext...| >>\n) {\n project: experience/project-evidence-flow\n requests >> project.request\n project.context >> contexts\n}\n";

fn revisions() -> (Vec<ExternalForeInput>, Vec<Vec<u8>>) {
    let mut inputs = Vec::new();
    let mut expected = Vec::new();
    for (revision, content, origin, role) in [
        (
            1,
            "The hallway light is on.",
            ExperienceOrigin::Observation,
            ExperienceTemporalRole::Current,
        ),
        (
            2,
            "The hallway light was on.",
            ExperienceOrigin::Remembered,
            ExperienceTemporalRole::Historical,
        ),
    ] {
        let provenance = format!("provenance/{revision}");
        let evidence = item(
            "light",
            origin,
            role,
            ExperienceAvailability::Present,
            revision,
            &provenance,
            content.as_bytes(),
        );
        let selection = ExperienceEvidenceSelection::new(
            ExperienceEvidenceIdentities::new(
                BoundedSequence::try_from_iter([identity("light")]).unwrap(),
            )
            .unwrap(),
            identity(&provenance),
            revision,
        )
        .unwrap();
        inputs.push(ExternalForeInput {
            front_port_id: "requests".into(),
            track: ConnectionTrack::Payload,
            bytes: ExperienceEvidenceRequest::new(items(vec![evidence.clone()]), selection.clone())
                .unwrap()
                .encode()
                .unwrap(),
        });
        expected.push(
            ExperienceEvidenceContext::new(items(vec![evidence]), selection)
                .unwrap()
                .encode()
                .unwrap(),
        );
    }
    (inputs, expected)
}

// This finite proof collector owns delivery bytes outside the kernel arena.
// Codec allocator guards separately establish allocation-free conversion.
struct Collector {
    delivered: Vec<ExternalForeDelivery>,
    stop: Option<RunControl>,
}
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        self.delivered.push(output);
        if let Some(control) = &self.stop {
            control
                .request_stop(RunControlRequestId::new("stop-after-context").unwrap())
                .map_err(|error| format!("{error:?}"))?;
        }
        Ok(())
    }
}

fn run(
    stop_after_first: bool,
    integrate: bool,
    prompt: bool,
) -> (crate::StdRunReport, Collector, Vec<Vec<u8>>) {
    let mut execution_host = host("evidence-flow");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_experience_evidence_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_experience_integration_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_experience_prompt_catalogs(&mut startup, &mut profile)
        .unwrap();
    let output_name = if prompt { "requests" } else { "contexts" };
    let (program, entry, input_name) = if prompt {
        (
            include_str!("../../../../plots/experiencer/model-context.conduit"),
            "experience-model-context",
            "updates",
        )
    } else if integrate {
        (
            include_str!("../../../../plots/experiencer/current.conduit"),
            "experiencer-current",
            "updates",
        )
    } else {
        (PROGRAM, "evidence-flow", "requests")
    };
    let parsed = parse_syntax_document(program);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, entry, &profile).unwrap();
    let hosts = [execution_host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundaries = [
        (input_name, PortDirection::Input),
        (output_name, PortDirection::Output),
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
                byte_capacity: 65_536,
            },
        )
    })
    .collect();
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 65_536,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(
        plan.fragments[0].placements.len(),
        if prompt {
            3
        } else if integrate {
            2
        } else {
            1
        }
    );
    assert_eq!(
        plan.fragments[0]
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == conduit_semantic_catalog::EXPERIENCE_EVIDENCE_FLOW_KIND)
            .unwrap()
            .implementation_id
            .as_str(),
        conduit_std_offers::EXPERIENCE_EVIDENCE_FLOW_IMPLEMENTATION
    );
    let control = RunControl::default();
    let mut collector = Collector {
        delivered: Vec::with_capacity(2),
        stop: stop_after_first.then(|| control.clone()),
    };
    let (inputs, expected) = if integrate {
        integration_inputs()
    } else {
        revisions()
    };
    let report = execution_host
        .run_external_plot_sequence_controlled_to(
            plan.fragments[0].clone(),
            &inputs,
            &mut collector,
            &mut Vec::new(),
            &mut RecordingTimer { waits: Vec::new() },
            &control,
        )
        .unwrap();
    (report, collector, expected)
}

#[test]
fn successive_experience_revisions_project_through_one_installed_gear_and_play() {
    let (report, collector, expected) = run(false, false, false);
    assert!(matches!(
        report.observations.last().map(|event| &event.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    assert_eq!(collector.delivered.len(), 2);
    for (delivery, expected) in collector.delivered.iter().zip(expected) {
        assert_eq!(delivery.front_port_id.as_str(), "contexts");
        assert_eq!(delivery.bytes, expected);
    }
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn cancellation_after_first_revision_does_not_emit_the_next_context() {
    let (report, collector, expected) = run(true, false, false);
    assert!(matches!(
        report.observations.last().map(|event| &event.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled { .. }
        })
    ));
    assert_eq!(collector.delivered.len(), 1);
    assert_eq!(collector.delivered[0].bytes, expected[0]);
    assert_eq!(report.control_receipts.len(), 1);
}

fn integration_inputs() -> (Vec<ExternalForeInput>, Vec<Vec<u8>>) {
    let observed = observation("light");
    let memory = with_sources(
        item(
            "memory",
            ExperienceOrigin::Remembered,
            ExperienceTemporalRole::Historical,
            ExperienceAvailability::Present,
            7,
            "provenance/7",
            b"Earlier the light was on",
        ),
        vec![ExperienceEvidenceSource::memory_record(identity("episode/1")).unwrap()],
    );
    let originals = [observed.clone(), memory.clone()];
    let mut inputs = Vec::new();
    let mut expected = Vec::new();
    for (revision, evidence) in [(0, observed), (1, memory)] {
        let encoded = ExperienceIntegrationUpdate::new(
            revision,
            ExperienceIntegrationOperation::upsert(
                *evidence.availability(),
                *evidence.certainty(),
                evidence.content_kind().clone(),
                *evidence.domain(),
                evidence.encoded_content().clone(),
                evidence.identity().clone(),
                *evidence.origin(),
                evidence.provenance_revision().clone(),
                *evidence.revision(),
                evidence.sources().clone(),
                *evidence.temporal_role(),
            )
            .unwrap(),
            identity(&format!("snapshot/{}", revision + 1)),
        )
        .unwrap()
        .encode()
        .unwrap();
        let provenance = identity(&format!("snapshot/{}", revision + 1));
        let expected_items = originals[..=revision as usize]
            .iter()
            .map(|item| {
                ExperienceEvidenceItem::new(
                    *item.availability(),
                    *item.certainty(),
                    item.content_kind().clone(),
                    *item.domain(),
                    item.encoded_content().clone(),
                    item.identity().clone(),
                    *item.origin(),
                    provenance.clone(),
                    revision + 1,
                    item.sources().clone(),
                    *item.temporal_role(),
                )
                .unwrap()
            })
            .collect();
        let selected = ExperienceEvidenceSelection::new(
            ExperienceEvidenceIdentities::new(
                BoundedSequence::try_from_iter(
                    originals[..=revision as usize]
                        .iter()
                        .map(|item| item.identity().clone()),
                )
                .unwrap(),
            )
            .unwrap(),
            provenance,
            revision + 1,
        )
        .unwrap();
        expected.push(
            ExperienceEvidenceContext::new(items(expected_items), selected)
                .unwrap()
                .encode()
                .unwrap(),
        );
        inputs.push(ExternalForeInput {
            front_port_id: "updates".into(),
            track: ConnectionTrack::Payload,
            bytes: encoded,
        });
    }
    (inputs, expected)
}

#[test]
fn ordinary_experiencer_plot_integrates_observation_and_recollection_in_one_play() {
    let (report, collector, expected) = run(false, true, false);
    assert_eq!(collector.delivered.len(), 2);
    for (delivered, expected) in collector.delivered.iter().zip(expected) {
        assert_eq!(delivered.bytes, expected);
    }
    let first = ExperienceEvidenceContext::decode(&collector.delivered[0].bytes).unwrap();
    let second = ExperienceEvidenceContext::decode(&collector.delivered[1].bytes).unwrap();
    assert_eq!(first.items().get().len(), 1);
    assert_eq!(second.items().get().len(), 2);
    assert_eq!(*second.selection().revision(), 2);
    assert_eq!(
        *second.items().get()[0].origin(),
        ExperienceOrigin::Observation
    );
    assert_eq!(
        *second.items().get()[1].origin(),
        ExperienceOrigin::Remembered
    );
    assert_eq!(
        *second.items().get()[1].temporal_role(),
        ExperienceTemporalRole::Historical
    );
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn cancelling_integrated_experience_stops_before_publishing_the_next_snapshot() {
    let (report, collector, expected) = run(true, true, false);
    assert_eq!(collector.delivered.len(), 1);
    assert_eq!(collector.delivered[0].bytes, expected[0]);
    assert!(matches!(
        report.observations.last().map(|event| &event.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled { .. }
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn continuing_experience_produces_bounded_model_context_without_promoting_memory() {
    let (report, collector, _) = run(false, true, true);
    assert_eq!(collector.delivered.len(), 2);
    let prompts = collector
        .delivered
        .iter()
        .map(|delivery| {
            assert_eq!(delivery.front_port_id.as_str(), "requests");
            assert!(delivery.bytes.len() <= conduit_semantic_catalog::EXPERIENCE_PROMPT_MAX_BYTES);
            serde_json::from_slice::<serde_json::Value>(&delivery.bytes).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(prompts[0]["evidence"]["selection"]["revision"], 1);
    assert_eq!(prompts[1]["evidence"]["selection"]["revision"], 2);
    let first = prompts[0]["evidence"]["items"].as_array().unwrap();
    let second = prompts[1]["evidence"]["items"].as_array().unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 2);
    assert_eq!(second[0]["origin"], "observation");
    assert_eq!(second[1]["origin"], "remembered");
    assert_eq!(second[1]["temporal_role"], "historical");
    assert_eq!(second[1]["content"], "Earlier the light was on");
    assert_eq!(second[1]["certainty"], "uncertain");
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}
