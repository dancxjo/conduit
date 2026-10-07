#![cfg(feature = "kernel-operation-owners")]
#[path = "fargan_epoch_contracts.rs"]
mod declarations;
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_pair_catalog::*, native_profile::PreparedNativeProfile,
    nominal_weakening::PreparedNominalWeakening,
};
use conduit_core::*;
use conduit_plot::*;
fn catalogs(capacity64: bool) -> (StartupCatalog, ProfileCatalog) {
    let mut s = StartupCatalog::new();
    let mut p = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut s, &mut p).unwrap();
    install_fixed_numeric_pair_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_flow::install_affine_flow_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_linear_flow::install_linear_flow_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_embedding_flow::install_embedding_flow_catalogs(&mut s, &mut p)
        .unwrap();
    if capacity64 {
        conduit_ai::fixed_numeric_temporal::install_closing_numeric_catalogs_capacity64(
            &mut s, &mut p,
        )
        .unwrap();
    } else {
        conduit_ai::fixed_numeric_temporal::install_closing_numeric_catalogs(&mut s, &mut p)
            .unwrap();
    }
    conduit_ai::fixed_numeric_pair_flow::install_fixed_flow_pair_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_dsp_catalog::install_fixed_dsp_flow_catalogs(&mut s, &mut p).unwrap();
    conduit_ai::fixed_numeric_integer_narrowing::install_checked_integer_flow_catalogs(
        &mut s, &mut p,
    )
    .unwrap();
    conduit_ai::fixed_numeric_float_integer::install_float_integer_catalogs(&mut s, &mut p)
        .unwrap();
    conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition(
        "type FarganPeriod = U16 in 32..=255\n",
    )
    .unwrap()
    .install(&mut s, &mut p, true)
    .unwrap();
    (s, p)
}
fn shape_contract(ty: &StructuredInfoType) -> CheckedValueContract {
    CheckedValueContract::new(
        ty.profile().unwrap().value_kind().clone(),
        maximum_prepared_transport_value_bytes(ty).unwrap(),
        vec![],
    )
    .unwrap()
}
struct EpochProfiles {
    startup: StartupCatalog,
    profiles: ProfileCatalog,
    imports: String,
    kinds: std::collections::BTreeMap<String, String>,
    native: Vec<std::sync::Arc<PreparedNativeProfile>>,
    weakening: Vec<std::sync::Arc<PreparedNominalWeakening>>,
    guards: Vec<conduit_ai::fixed_numeric_guard::FixedGuardProfile>,
    zip: conduitos::flow_zip::FlowZipOperationFactory,
}
fn prepared_epoch_profiles() -> EpochProfiles {
    prepared_epoch_profiles_with_capacity(false)
}
fn prepared_epoch_profiles_with_capacity(capacity64: bool) -> EpochProfiles {
    let (mut startup, mut profiles) = catalogs(capacity64);
    let definition = declarations::exact_epoch_declarations();
    let types =
        check_syntax_document(&parse_syntax_document(&definition), &StartupCatalog::new()).unwrap();
    let ty = |name: &str| {
        types
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type
            .clone()
    };
    let mut native = vec![];
    let mut weakening = vec![];
    let mut zip = conduitos::flow_zip::FlowZipOperationFactory::frame16k();
    let mut imports = String::new();
    let mut kinds = std::collections::BTreeMap::new();
    for (key, name) in [
        ("EPOCH_INPUT", "FarganFloatEpochInput"),
        ("PHASE1", "FarganFloatPhase1"),
        ("PHASE2", "FarganFloatPhase2"),
        ("PHASE3", "FarganFloatPhase3"),
        ("PHASE4", "FarganFloatEpochProposal"),
        ("PCM", "FarganPcm16EpochResult"),
    ] {
        let profile = std::sync::Arc::new(
            PreparedNativeProfile::check_definition(&definition, name).unwrap(),
        );
        profile.install(&mut startup, &mut profiles, true).unwrap();
        let id = profile.kind_identity(true);
        kinds.insert(format!("__{key}_VALIDATOR__"), id.clone());
        if key.starts_with("PHASE") || key == "PCM" {
            let alias = if key == "PCM" {
                "FarganPcm16Candidate".to_owned()
            } else {
                format!("FarganPhase{}Candidate", &key[5..])
            };
            imports.push_str(&format!("with {id}/candidate as {alias}\n"));
        }
        native.push(profile);
    }
    let guard =
        conduit_ai::fixed_numeric_guard::FixedGuardProfile::prepare(ty("FarganFloatEpochInput"))
            .unwrap();
    guard.install(&mut startup, &mut profiles).unwrap();
    kinds.insert(
        "__EPOCH_GUARD__".into(),
        guard.contract().unwrap().kind_id.as_str().to_owned(),
    );
    let entry = PreparedNominalWeakening::prepare(ty("FarganFloatEpochInput")).unwrap();
    entry.install(&mut startup, &mut profiles, true).unwrap();
    kinds.insert(
        "__EPOCH_INPUT_WEAKENING__".into(),
        entry.kind_identity(true),
    );
    weakening.push(std::sync::Arc::new(entry));
    for (key, left, right, alias) in [
        (
            "PHASE",
            ty("FarganEpochPhaseCarry"),
            ty("FarganSubframeResult"),
            "FarganEpochPhasePair",
        ),
        (
            "PCM",
            ty("FarganEpochFinalCarry"),
            fixed_numeric_type("NumericI16Vector160").unwrap(),
            "FarganPcm16Pair",
        ),
    ] {
        let id = conduit_semantic_catalog::install_flow_zip_finite_specialized_kind(
            &shape_contract(&left),
            &left,
            &shape_contract(&right),
            &right,
            &mut startup,
            &mut profiles,
        )
        .unwrap();
        let selected = zip
            .install_specialized(
                &shape_contract(&left),
                &left,
                &shape_contract(&right),
                &right,
            )
            .unwrap();
        assert_eq!(selected.kind_id, id);
        kinds.insert(format!("__{key}_ZIP__"), id.as_str().to_owned());
        let pair = PreparedTypedTuplePairEncoder::new(
            left.clone(),
            maximum_prepared_transport_value_bytes(&left).unwrap(),
            right.clone(),
            maximum_prepared_transport_value_bytes(&right).unwrap(),
        )
        .unwrap();
        let weak = PreparedNominalWeakening::prepare(pair.value_type().clone()).unwrap();
        weak.install(&mut startup, &mut profiles, true).unwrap();
        kinds.insert(format!("__{key}_WEAKENING__"), weak.kind_identity(true));
        imports.push_str(&format!(
            "with {}/result as {alias}\n",
            weak.kind_identity(true)
        ));
        weakening.push(std::sync::Arc::new(weak));
    }
    EpochProfiles {
        startup,
        profiles,
        imports,
        kinds,
        native,
        weakening,
        guards: vec![guard],
        zip,
    }
}
#[test]
fn authored_epoch_imports_and_owners_match_exact_checked_profiles() {
    let context = prepared_epoch_profiles();
    let imports = context.imports;
    let kinds = context.kinds;
    let source = include_str!("../../speech/fargan_epoch_flow.conduit");
    for import in imports.lines() {
        assert!(source.lines().any(|line| line == import), "{import}");
    }
    for identity in kinds.values() {
        assert!(source.contains(identity), "{identity}");
    }
    assert!(!source.contains("__"));
}

#[test]
fn complete_four_phase_epoch_checks_and_expands_exact_closing_flow_owners() {
    let context = prepared_epoch_profiles();
    assert_eq!(context.native.len(), 6);
    assert_eq!(context.weakening.len(), 3);
    assert_eq!(context.guards.len(), 1);
    assert_eq!(context.zip.offers().count(), 2);
    let startup = context.startup;
    let profiles = context.profiles;
    let source = epoch_source();
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "speech/flow-fargan-float-epoch", &profiles)
            .unwrap();
    eprintln!(
        "complete epoch: {} nodes, {} cords",
        expanded.expanded.gears.len(),
        expanded.expanded.connections.len()
    );
    let mut multiplicities = std::collections::BTreeMap::new();
    for gear in &expanded.expanded.gears {
        *multiplicities
            .entry(gear.kind_id.as_str())
            .or_insert(0usize) += 1;
    }
    for (kind, count) in multiplicities.iter().filter(|(_, count)| **count > 16) {
        eprintln!("epoch multiplicity: {kind} = {count}");
    }
    assert!(expanded.expanded.gears.len() > 600);
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "numeric/flow-dense128x40")
            .count(),
        4
    );
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|gear| gear
                .kind_id
                .as_str()
                .starts_with("structure/flow-native-profile/"))
            .count(),
        6
    );
    for gear in &expanded.expanded.gears {
        for port in gear.inputs.iter().chain(&gear.outputs) {
            assert_eq!(
                port.temporal,
                if ["weights", "bias"].contains(&port.port_id.as_str()) {
                    PortTemporal::Value
                } else {
                    PortTemporal::Flow { closes: true }
                },
                "{}:{}",
                gear.kind_id.as_str(),
                port.port_id.as_str()
            );
        }
    }
}

fn epoch_source() -> String {
    let mut source = format!(
        "{}\n{}\n{}",
        include_str!("../../speech/fargan_epoch_flow.conduit"),
        (declarations::exact_epoch_declarations()
            + "\n"
            + &declarations::declarations_only(include_str!(
                "../../speech/fargan_pitch_history.conduit"
            )))
            .lines()
            .filter(|line| !line.starts_with("type Numeric"))
            .collect::<Vec<_>>()
            .join("\n"),
        [
            include_str!("../../speech/fargan_signal_flow.conduit"),
            include_str!("../../speech/fargan_pitch_history_flow.conduit"),
            include_str!("../../speech/fargan_subframe_flow.conduit"),
            include_str!("../../speech/fargan_epoch_projections.conduit"),
            include_str!("../../speech/fargan_epoch_policy.conduit"),
            include_str!("../../speech/fargan_epoch_anchor.conduit"),
            include_str!("../../speech/fargan_epoch_merges.conduit"),
        ]
        .join("\n")
    );
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let selected=format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    source = source.replace(
        "selected: FarganModelFrameAnchor",
        &format!("selected: FarganModelFrameAnchor = {selected}"),
    );
    source
}

fn numeric_epoch_offer(kind: &str, capacity64: bool) -> CapabilityOffer {
    use conduit_ai::operation_owners::*;
    (if capacity64 {
        fixed_numeric::fixed_numeric_offer_capacity64(kind)
    } else {
        fixed_numeric::fixed_numeric_offer(kind)
    })
    .or_else(|_| fixed_numeric_flow::fixed_affine_flow_offer(kind))
    .or_else(|_| fixed_numeric_linear_flow::fixed_linear_flow_offer(kind))
    .or_else(|_| fixed_numeric_embedding_flow::fixed_embedding_flow_offer(kind))
    .or_else(|_| conduit_ai::fixed_numeric_pair_flow::fixed_flow_pair_offer(kind))
    .or_else(|_| {
        if kind == "numeric/flow-f32-to-i16-nearest-away160" {
            conduit_ai::fixed_numeric_float_integer::float_integer_offer(true)
        } else {
            Err(format!("unsupported exact numeric epoch owner {kind}"))
        }
    })
    .unwrap_or_else(|e| panic!("{kind}: {e}"))
}

fn prepared_epoch_plan(
    capacity64: bool,
) -> Result<(Plan, EpochProfiles), conduit_planner::PlannerError> {
    let mut context = prepared_epoch_profiles_with_capacity(capacity64);
    let source = epoch_source();
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let mut types: std::collections::BTreeMap<_, _> = fixed_numeric_types()
        .unwrap()
        .into_iter()
        .map(|ty| (ty.name, ty.value_type))
        .collect();
    types.extend(
        checked
            .native_types
            .iter()
            .map(|ty| (ty.name.clone(), ty.value_type.clone())),
    );
    let entry = checked
        .plots
        .iter()
        .find(|p| p.name == "speech/flow-fargan-float-epoch")
        .unwrap();
    let mut offers: Vec<_> = context
        .native
        .iter()
        .map(|p| p.offer(true).unwrap())
        .collect();
    offers.extend(context.weakening.iter().map(|p| p.offer(true).unwrap()));
    offers.extend(context.guards.iter().map(|p| p.offer().unwrap()));
    offers.extend(context.zip.offers().cloned());
    let period = conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition(
        "type FarganPeriod = U16 in 32..=255\n",
    )
    .unwrap();
    offers.push(period.offer(true).unwrap());
    let mut wrapper =
        String::from("\nplot epoch-runtime-proof {\n inner: speech/flow-fargan-float-epoch\n");
    for port in &entry.runtime_ports {
        let name = format!("epoch-proof/{}", port.name.text);
        let input = port.direction == conduit_plot::syntax::RuntimePortDirection::Input;
        let ty = &types[&port.value_type.text];
        let descriptor = PortDescriptor {
            port_id: port_id("value"),
            value_kind: ty.profile().unwrap().value_kind().clone(),
            direction: if input {
                PortDirection::Output
            } else {
                PortDirection::Input
            },
            temporal: if ["value", "result"].contains(&port.name.text.as_str()) {
                PortTemporal::Flow { closes: true }
            } else {
                PortTemporal::Value
            },
            abnormal_kind: None,
        };
        let kind = Kind {
            kind_id: kind_id(&name),
            kind_contract_revision: name.clone().into(),
            startup_parameters: vec![],
            shorthand: None,
            configuration: vec![],
            inputs: if input {
                vec![]
            } else {
                vec![descriptor.clone()]
            },
            outputs: if input {
                vec![descriptor.clone()]
            } else {
                vec![]
            },
            semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: if input {
                    FrontValueLocation::Output(descriptor.port_id)
                } else {
                    FrontValueLocation::Input(descriptor.port_id)
                },
                contract: shape_contract(ty),
            }])],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: maximum_prepared_transport_value_bytes(ty).unwrap(),
            },
        };
        context
            .startup
            .insert(KindSignature {
                kind: name.clone(),
                startup_parameters: vec![],
            })
            .unwrap();
        context.profiles.insert_kind(kind.clone()).unwrap();
        offers.push(
            BackOfferBuilder::new(
                kind,
                Back {
                    capability_id: name.clone().into(),
                    execution_profile_id: "synthetic-epoch-driver@1".into(),
                    implementation_id: name.clone().into(),
                    artifact_id: name.clone().into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build(),
        );
        wrapper.push_str(&format!(" {}: {name}\n", port.name.text));
        wrapper.push_str(&if input {
            format!(" {}.value >> inner.{}\n", port.name.text, port.name.text)
        } else {
            format!(" inner.{} >> {}.value\n", port.name.text, port.name.text)
        });
    }
    wrapper.push_str("}\n");
    let checked = check_syntax_document(
        &parse_syntax_document(&format!("{source}{wrapper}")),
        &context.startup,
    )
    .unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "epoch-runtime-proof", &context.profiles)
            .unwrap();
    assert!(expanded.input_bindings.is_empty() && expanded.output_bindings.is_empty());
    for gear in &expanded.expanded.gears {
        let selected = if gear.kind_id.as_str().starts_with("numeric/") {
            Some(numeric_epoch_offer(gear.kind_id.as_str(), capacity64))
        } else if let [entry] = gear.configuration.as_slice() {
            let ConfigurationValue::Text(encoded) = &entry.value else {
                panic!("pure source program")
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            Some(
                conduitos::expression_host_call::offer(
                    &program,
                    PortTemporal::Flow { closes: true },
                )
                .unwrap(),
            )
        } else {
            None
        };
        if let Some(selected) = selected {
            if !offers
                .iter()
                .any(|offer| offer.capability_id == selected.capability_id)
            {
                offers.push(selected);
            }
        }
    }
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "epoch-runtime-proof".into(),
        boot_id: "epoch-proof-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "synthetic-floating-epoch@1".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    };
    let placements = conduit_planner::default_expanded_placements(
        &expanded.expanded,
        std::slice::from_ref(&host),
    )?;
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded.expanded,
        &[host],
        &placements,
        &["conduit.base/local@1".into()],
    )?;
    assert!(verify_plan(&plan));
    conduit_ai::operation_owners::native_profile::NativeProfileOperationFactory::for_plan(
        &plan,
        &context.native,
    )
    .unwrap();
    conduit_ai::operation_owners::nominal_weakening::NominalWeakeningOperationFactory::for_plan(
        &plan,
        &context.weakening,
    )
    .unwrap();
    conduit_ai::operation_owners::fixed_numeric_guard::FixedGuardOperationFactory::for_plan(
        &plan,
        context.guards.clone(),
    )
    .unwrap();
    eprintln!(
        "ordinary epoch Plan: {} placements, {} cords",
        plan.fragments[0].placements.len(),
        expanded.expanded.connections.len()
    );
    Ok((plan, context))
}

#[test]
fn default_numeric_capacity_refuses_four_simultaneous_epoch_phases() {
    match prepared_epoch_plan(false) {
        Err(conduit_planner::PlannerError::UnknownCapability(kind)) => {
            assert!(matches!(
                kind.as_str(),
                "numeric/flow-add128"
                    | "numeric/flow-multiply128"
                    | "numeric/flow-scale40"
                    | "numeric/flow-sigmoid128"
                    | "numeric/flow-slice384x128"
                    | "numeric/flow-slice480x160"
            ));
        }
        Err(error) => panic!("unexpected preparation refusal: {error:?}"),
        Ok(_) => panic!("default sixteen-instance profile must refuse the four-phase graph"),
    }
}

#[test]
fn signal_feedback_and_condition_event_fit_exact_selected_pair_profile() {
    let definition = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../speech/fargan_epoch_feedback.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(&definition), &StartupCatalog::new()).unwrap();
    let find = |name| {
        checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
            .clone()
    };
    let state = find("FarganSignalEpochFeedback");
    let event = find("FarganSignalConditionEpoch");
    let pair = PreparedTypedTuplePairEncoder::new(
        state.clone(),
        maximum_prepared_transport_value_bytes(&state).unwrap(),
        event.clone(),
        maximum_prepared_transport_value_bytes(&event).unwrap(),
    )
    .unwrap();
    eprintln!(
        "Signal cycle: state={}, event={}, pair={}",
        maximum_prepared_transport_value_bytes(&state).unwrap(),
        maximum_prepared_transport_value_bytes(&event).unwrap(),
        pair.maximum_bytes()
    );
    assert!(pair.maximum_bytes() as usize <= MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    let profile =
        PreparedNativeProfile::check_definition(&definition, "FarganSignalEpochFeedback").unwrap();
    assert_eq!(profile.value_type(), &state);
}

fn prepared_signal_cycle_profiles() -> (
    EpochProfiles,
    conduitos::seeded_state::SeededStateOperationFactory,
    std::collections::BTreeMap<String, String>,
) {
    let mut context = prepared_epoch_profiles();
    let definition = declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../speech/fargan_epoch_feedback.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(&definition), &StartupCatalog::new()).unwrap();
    let find = |name| {
        checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
            .clone()
    };
    let state = find("FarganSignalEpochFeedback");
    let event = find("FarganSignalConditionEpoch");
    let mut ids = std::collections::BTreeMap::new();
    let mut seeded = conduitos::seeded_state::SeededStateOperationFactory::default();
    let kind = conduit_semantic_catalog::install_seeded_state_flow_specialized_kind(
        &shape_contract(&state),
        &state,
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    let offer = seeded
        .install_flow_specialized_frame16k(&shape_contract(&state), &state)
        .unwrap();
    assert_eq!(offer.kind_id, kind);
    ids.insert("__SIGNAL_CELL__".into(), kind.as_str().to_owned());
    let pair_kind = conduit_semantic_catalog::install_flow_zip_feedback_specialized_kind(
        &shape_contract(&state),
        &state,
        &shape_contract(&event),
        &event,
        &mut context.startup,
        &mut context.profiles,
    )
    .unwrap();
    let offer = context
        .zip
        .install_feedback_specialized(
            &shape_contract(&state),
            &state,
            &shape_contract(&event),
            &event,
        )
        .unwrap();
    assert_eq!(offer.kind_id, pair_kind);
    ids.insert("__SIGNAL_ZIP__".into(), pair_kind.as_str().to_owned());
    let pair = PreparedTypedTuplePairEncoder::new(
        state.clone(),
        maximum_prepared_transport_value_bytes(&state).unwrap(),
        event.clone(),
        maximum_prepared_transport_value_bytes(&event).unwrap(),
    )
    .unwrap();
    let weak =
        std::sync::Arc::new(PreparedNominalWeakening::prepare(pair.value_type().clone()).unwrap());
    weak.install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert("__SIGNAL_PAIR_WEAK__".into(), weak.kind_identity(true));
    let guard =
        conduit_ai::fixed_numeric_guard::FixedGuardProfile::prepare(weak.output_type().clone())
            .unwrap();
    guard
        .install(&mut context.startup, &mut context.profiles)
        .unwrap();
    ids.insert(
        "__SIGNAL_GUARD__".into(),
        guard.contract().unwrap().kind_id.as_str().to_owned(),
    );
    context.guards.push(guard);
    context.weakening.push(weak);
    let nextweak = std::sync::Arc::new(
        PreparedNominalWeakening::prepare(
            context
                .native
                .iter()
                .find(|p| p.kind_identity(true) == context.kinds["__PCM_VALIDATOR__"])
                .unwrap()
                .value_type()
                .clone(),
        )
        .unwrap(),
    );
    nextweak
        .install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert("__SIGNAL_NEXT_WEAK__".into(), nextweak.kind_identity(true));
    context.weakening.push(nextweak);
    let stateprofile = std::sync::Arc::new(
        PreparedNativeProfile::check_definition(&definition, "FarganSignalEpochFeedback").unwrap(),
    );
    stateprofile
        .install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert(
        "__SIGNAL_STATE_PROFILE__".into(),
        stateprofile.kind_identity(true),
    );
    ids.insert(
        "__SIGNAL_EPOCH_PROFILE__".into(),
        context.kinds["__EPOCH_INPUT_VALIDATOR__"].clone(),
    );
    context.native.push(stateprofile);
    (context, seeded, ids)
}

#[test]
fn signal_cycle_exact_specializations_retain_seed_feedback_and_full_admission() {
    let (context, seeded, ids) = prepared_signal_cycle_profiles();
    assert_eq!(seeded.offers().count(), 1);
    assert_eq!(context.zip.offers().count(), 3);
    assert_eq!(context.native.len(), 7);
    assert_eq!(context.weakening.len(), 5);
    assert_eq!(context.guards.len(), 2);
    eprintln!(
        "signal cycle identities: {}",
        serde_json::to_string(&ids).unwrap()
    );
}

#[test]
fn authored_signal_feedback_cycle_checks_exact_epoch_guard_and_final_pcm_ack() {
    let (context, _, ids) = prepared_signal_cycle_profiles();
    let cycle = include_str!("../../speech/fargan_signal_cycle.conduit");
    for identity in ids.values() {
        assert!(cycle.contains(identity), "{identity}");
    }
    let imports = cycle
        .lines()
        .filter(|line| line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let body = cycle
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let selected=format!("{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}");
    let body = body.replace(
        "selected: FarganModelFrameAnchor\n",
        &format!("selected: FarganModelFrameAnchor = {selected}\n"),
    );
    let source = format!(
        "{imports}\n{}\n{}\n{body}",
        epoch_source(),
        include_str!("../../speech/fargan_epoch_feedback.conduit")
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let matcher = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-signal-cycle-matches",
        &context.profiles,
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &matcher.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for (next, event, accepted) in [
        (0, 0, true),
        (7, 7, true),
        (7, 8, false),
        (u64::MAX - 1, u64::MAX - 1, true),
        (u64::MAX, u64::MAX, false),
    ] {
        let input = epoch_pair_fixture(&program.input_type, "", next, event)
            .canonical_bytes()
            .unwrap();
        let expected = InfoBool::new(accepted).encode();
        assert_eq!(program.evaluate(&input).unwrap(), expected);
        assert_eq!(prepared.evaluate(&input).unwrap(), expected);
    }
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-signal-cycle",
        &context.profiles,
    )
    .unwrap();
    eprintln!(
        "Signal epoch cycle: {} nodes, {} cords",
        expanded.expanded.gears.len(),
        expanded.expanded.connections.len()
    );
    assert!(expanded.expanded.gears.len() > 742);
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == ids["__SIGNAL_CELL__"])
            .count(),
        1
    );
    assert_eq!(
        expanded
            .expanded
            .gears
            .iter()
            .filter(|g| g.kind_id.as_str() == ids["__SIGNAL_ZIP__"])
            .count(),
        1
    );
}

fn epoch_pair_fixture(
    ty: &StructuredInfoType,
    name: &str,
    next: u64,
    event: u64,
) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => StructuredInfoValue::nominal(
            ty.clone(),
            epoch_pair_fixture(representation, name, next, event),
        )
        .unwrap(),
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        epoch_pair_fixture(field.value_type(), field.name(), next, event),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/u64" => {
            StructuredInfoValue::leaf(
                ty.clone(),
                if name == "next_epoch" { next } else { event }
                    .to_le_bytes()
                    .to_vec(),
            )
            .unwrap()
        }
        _ => declarations::fixture_value(ty),
    }
}

#[test]
fn explicit_capacity64_epoch_plan_selects_exact_generic_owners() {
    let (plan, context) = prepared_epoch_plan(true).unwrap();
    assert_eq!(context.native.len(), 6);
    assert_eq!(context.weakening.len(), 3);
    assert_eq!(context.zip.offers().count(), 2);
    assert!(plan.fragments[0].placements.len() > 742);
}
