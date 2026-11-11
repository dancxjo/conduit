//! Preparation for a live model proof using explicitly scripted human statements.
//! This does not acquire physical observations or inject remembered evidence.
use conduit_core::{ConnectionTrack, Plan, PortDirection, SignId, TemporalInstant, TemporalScale};
use conduit_pete::{pete_situation_frame, select_pete_situation, Observed, SituationInput};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_connection_limits,
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{ExternalForeInput, StdHost};
use std::collections::BTreeMap;

pub(super) const FIRST_OBSERVATION: &str =
    "I put a cobalt-blue mug on the desk. Please remember its color.";
pub(super) const SECOND_OBSERVATION: &str = "What color mug did I tell you I put on the desk?";

pub(super) fn plan(host: &StdHost, cognition: bool) -> Result<Plan, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    let (source, entry) = if cognition {
        conduit_pete::install_pete_situated_cognition_catalogs(&mut startup, &mut profile)?;
        (
            conduit_pete::PETE_SITUATED_COGNITION_PLOT_SOURCE,
            "pete-situated-cognition",
        )
    } else {
        conduit_time::install_time_every_catalog(&mut startup, &mut profile)?;
        conduit_semantic_catalog::install_tick_face_catalog(&mut startup, &mut profile)?;
        (
            include_str!("../../../../../plots/clock/main.conduit"),
            "clock-demo",
        )
    };
    let parsed = parse_syntax_document(source);
    if !parsed.diagnostics.is_empty() {
        return Err(format!("parse {entry}: {:?}", parsed.diagnostics));
    }
    let checked = check_syntax_document(&parsed, &startup)
        .map_err(|error| format!("check {entry}: {error:?}"))?;
    let authoring = expand_canonical_plot_for_authoring(&checked, entry, &profile)
        .map_err(|error| format!("expand {entry}: {error:?}"))?;
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts)
        .map_err(|error| format!("place {entry}: {error:?}"))?;
    let boundaries: BTreeMap<_, _> = if cognition {
        [
            ("observations", PortDirection::Input, 65_536),
            ("situations", PortDirection::Output, 65_536),
            ("responses", PortDirection::Output, 16_384),
            ("autobiography", PortDirection::Output, 65_536),
            ("losses", PortDirection::Output, 34),
        ]
        .into_iter()
        .map(|(name, direction, bytes)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: name.into(),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            )
        })
        .collect()
    } else {
        BTreeMap::new()
    };
    let connections = authoring
        .expanded
        .connections
        .iter()
        .map(|cord| {
            let bytes = match cord.value_kind.as_str() {
                conduit_core::TERMINAL_INFO_ID => 34,
                conduit_ai::GENERATED_RESULT_VALUE_KIND
                | conduit_ai::GENERATION_REQUEST_VALUE_KIND => 16_384,
                _ if cognition => 65_536,
                _ => 64,
            };
            (
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            )
        })
        .collect();
    plan_expanded_authoring_with_connection_limits(
        &authoring,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: if cognition { 65_536 } else { 64 },
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connections,
        &boundaries,
    )
    .map_err(|error| format!("plan {entry}: {error:?}"))
}

pub(super) fn inputs() -> Result<Vec<ExternalForeInput>, String> {
    [
        (FIRST_OBSERVATION, 1_000, "scripted-human/first", "first"),
        (SECOND_OBSERVATION, 2_000, "scripted-human/second", "second"),
    ]
    .into_iter()
    .map(|(text, ticks, sign, provenance)| {
        let reference = TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "clock/scripted-pete-cognition".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        };
        let situation = select_pete_situation(
            reference.clone(),
            10,
            SituationInput::Missing,
            SituationInput::Observed(Observed {
                value: text.into(),
                sign_id: SignId::from(sign),
                observed_at: reference,
            }),
            SituationInput::Missing,
        )
        .map_err(|error| format!("prepare scripted {provenance} Situation: {error:?}"))?;
        let policy = conduit_human::ExperienceTemporalPolicy::new(10, 20)
            .map_err(|error| format!("scripted freshness policy: {error:?}"))?;
        let frame = pete_situation_frame(&situation, policy, 0, provenance, true)
            .map_err(|error| format!("prepare scripted {provenance} frame: {error:?}"))?;
        let bytes = frame
            .encode()
            .map_err(|error| format!("encode scripted {provenance} frame: {error:?}"))?;
        Ok(ExternalForeInput {
            front_port_id: "observations".into(),
            track: ConnectionTrack::Payload,
            bytes,
        })
    })
    .collect()
}
