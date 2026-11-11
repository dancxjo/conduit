use super::*;
pub(super) fn plan(host: &StdHost, cognition: bool, recovery: bool) -> Plan {
    plan_kind(host, cognition, recovery, false)
}
pub(super) fn situated(host: &StdHost) -> Plan {
    plan_kind(host, true, true, true)
}
fn plan_kind(host: &StdHost, cognition: bool, recovery: bool, situated: bool) -> Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_experience_evidence_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_experience_integration_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_experience_prompt_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_experience_autobiography_catalogs(&mut startup, &mut profile)
        .unwrap();
    conduit_semantic_catalog::install_experience_autobiography_recall_catalogs(
        &mut startup,
        &mut profile,
    )
    .unwrap();
    install_llm_semantic_catalog(&mut startup, &mut profile).unwrap();
    if situated {
        conduit_semantic_catalog::install_experience_situation_catalogs(&mut startup, &mut profile)
            .unwrap();
    }
    if recovery {
        conduit_semantic_catalog::install_experience_autobiography_recovery_catalogs(
            &mut startup,
            &mut profile,
        )
        .unwrap();
        startup
            .insert_value_kind_alias(
                "PeteModelLoss",
                conduit_core::kind_id(conduit_core::TERMINAL_INFO_ID),
            )
            .unwrap();
    }
    startup
        .insert_value_kind_alias(
            "PeteCognitionResponse",
            conduit_core::kind_id(GENERATED_RESULT_VALUE_KIND),
        )
        .unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let initial = conduit_semantic_catalog::initial_experience_autobiography_state()
        .into_structured()
        .unwrap();
    conduit_semantic_catalog::state_value::install_state_value_kind(
        "ExperienceAutobiographyState",
        initial.value_type(),
        &initial,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    conduit_time::install_time_every_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_face_catalog(&mut startup, &mut profile).unwrap();
    let (source, entry) = if situated {
        (
            include_str!("../../../../../../plots/pete-cognition/situated.conduit"),
            "pete-situated-cognition",
        )
    } else if recovery && cognition {
        (
            include_str!("../../../../../../plots/pete-cognition/resilient.conduit"),
            "pete-resilient-cognition",
        )
    } else if cognition {
        (
            include_str!("../../../../../../plots/pete-cognition/autobiographical.conduit"),
            "pete-autobiographical-cognition",
        )
    } else {
        (
            include_str!("../../../../../../plots/clock/main.conduit"),
            "clock-demo",
        )
    };
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, entry, &profile).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundaries = [
        ("events", PortDirection::Input, 65_536),
        ("observations", PortDirection::Input, 65_536),
        ("situations", PortDirection::Output, 65_536),
        ("responses", PortDirection::Output, 16_384),
        ("autobiography", PortDirection::Output, 65_536),
        ("losses", PortDirection::Output, 34),
    ]
    .into_iter()
    .filter(|(name, _, _)| match *name {
        "losses" => recovery,
        "events" => !situated,
        "observations" | "situations" => situated,
        _ => true,
    })
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
    .collect();
    let no_boundaries = BTreeMap::new();
    let connections = authoring
        .expanded
        .connections
        .iter()
        .map(|cord| {
            let bytes = if cord.value_kind.as_str() == conduit_core::TERMINAL_INFO_ID {
                34
            } else if cord.value_kind.as_str() == GENERATED_RESULT_VALUE_KIND
                || cord.value_kind.as_str() == GENERATION_REQUEST_VALUE_KIND
            {
                16_384
            } else if cognition {
                65_536
            } else {
                64
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
        if cognition {
            &boundaries
        } else {
            &no_boundaries
        },
    )
    .unwrap()
}
