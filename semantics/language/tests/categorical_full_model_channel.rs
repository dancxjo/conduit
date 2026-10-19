//! Isolate full413/76/25 ordinary numeric kernel admission from Source preparation.
#![cfg(all(
    feature = "parser-model",
    target_has_atomic = "ptr",
    target_has_atomic = "8"
))]
#[path = "common/parser_round_model_channel.rs"]
mod numeric;
use conduit_ai::integer_categorical_step::channel::*;
use conduit_core::*;
#[test]
fn full_parser_model_channel_admits_canonical_features_and_retains_ack() {
    let profile = numeric::profile();
    assert_eq!(profile.dimensions(), (413, 76, 25));
    let (document, catalog, definition) = numeric::definition(&profile);
    let allowance = definition.external_capability.limits.max_queue_bytes;
    let fores: Vec<_> = definition
        .internal_plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.fore_ports)
        .map(|fore| {
            (
                fore.front_port_id.clone(),
                fore.byte_capacity,
                fore.value_contract.clone(),
            )
        })
        .collect();
    let StructuredInfoTypeShape::Collection { element, .. } = profile.indices_type().shape() else {
        panic!("indices");
    };
    let input = StructuredInfoValue::collection(
        profile.indices_type().clone(),
        (0..25)
            .map(|_| {
                StructuredInfoValue::leaf(element.clone(), 0u64.to_le_bytes().to_vec()).unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    assert!(
        input.len() >= 200,
        "regress beyond the incidental64B toy profile"
    );
    let mut channel = PreparedCategoricalChannel::prepare_canonical(
        &document,
        "parser-round-numeric",
        &catalog,
        definition,
        profile,
        1,
    )
    .unwrap();
    match channel.submit((),&input) {
        Ok(CategoricalChannelSubmission::Accepted) => (),
        other => panic!("actual ingress {other:?}; underlying {:?}; input {}B; exported allowance {allowance}B; sealed fores {fores:?}",
            channel.last_kernel_error(),input.len()),
    }
    for _ in 0..16 {
        match channel.step() {
            Ok(Some(done)) => {
                let (_, receipt) = done.into_parts();
                let mut retained = vec![0; receipt.input_len()];
                receipt.copy_input_into(&mut retained).unwrap();
                assert_eq!(retained, input);
                assert_eq!(receipt.invocation(), 1);
                let mut output = vec![0; receipt.output_len()];
                receipt.copy_output_into(&mut output).unwrap();
                let value = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
                let StructuredInfoValueShape::Collection(values) = value.shape() else {
                    panic!("scores");
                };
                assert_eq!(values.len(), 76);
                assert!(!receipt.active_plays().is_empty());
                assert!(!channel.signs().is_empty());
                return;
            }
            Ok(None) => (),
            Err(refusal) => panic!(
                "actual step {refusal:?}; underlying {:?}",
                channel.last_kernel_error()
            ),
        }
    }
    panic!("bounded actual numerical request did not complete")
}

fn reseal_for_refusal(plan: Plan) -> Plan {
    // Deliberately re-seal malformed test candidates so rejection must inspect
    // canonical Source/Fore correspondence, not just a stale Plan digest.
    seal_plan_with_completion(
        PlotIdentity {
            source_document_id: plan.source_document_id,
            checked_plot_id: plan.checked_plot_id,
            expanded_plot_id: plan.expanded_plot_id,
        },
        plan.completion_policy,
        plan.fragments,
    )
}
fn rebuild(
    document: &conduit_plot::CheckedSyntaxDocument,
    catalog: &conduit_plot::ProfileCatalog,
    definition: &conduit_composite::KernelCompositeDefinition,
    plan: Plan,
) -> Result<
    conduit_composite::KernelCompositeDefinition,
    conduit_composite::KernelCompositeDefinitionError,
> {
    let cap = &definition.external_capability;
    conduit_composite::KernelCompositeDefinition::from_canonical_export(
        definition.host_id.clone(),
        definition.boot_id.clone(),
        definition.offer_generation,
        definition.profile.clone(),
        cap.implementation.implementation_id.clone(),
        cap.implementation.artifact_id.clone(),
        document,
        "parser-round-numeric",
        catalog,
        plan,
        definition.failure_translation.clone(),
    )
}
#[test]
fn canonical_export_rejects_foreign_source_and_resealed_missing_extra_queue_contract_forgeries() {
    let profile = numeric::profile();
    let (document, catalog, definition) = numeric::definition(&profile);
    let mut foreign = document.clone();
    foreign.source_document_id = "source/foreign-canonical-document".into();
    assert!(rebuild(
        &foreign,
        &catalog,
        &definition,
        definition.internal_plan.clone()
    )
    .is_err());
    let mut fore_source_checks = 0;
    let mut fore_core_refusals = 0;
    for alteration in 0..5 {
        let mut plan = definition.internal_plan.clone();
        let fores = &mut plan.fragments[0].fore_ports;
        match alteration {
            0 => {
                fores.pop();
            }
            1 => fores.push(fores[0].clone()),
            2 => fores[0].byte_capacity = 1,
            3 => fores[0].value_contract.as_mut().unwrap().maximum_bytes = 1,
            4 => fores[0].track = ConnectionTrack::NormalClose,
            _ => unreachable!(),
        }
        let plan = reseal_for_refusal(plan);
        if verify_plan(&plan) {
            fore_source_checks += 1;
        } else {
            fore_core_refusals += 1;
        }
        assert!(
            rebuild(&document, &catalog, &definition, plan).is_err(),
            "forgery {alteration}"
        );
    }
    let mut graph_source_checks = 0;
    let mut graph_core_refusals = 0;
    for alteration in 0..3 {
        let mut plan = definition.internal_plan.clone();
        let gear = &mut plan.fragments[0].placements[0];
        match alteration {
            0 => gear.gear_id = "forged/internal-gear".into(),
            1 => gear.kind_id = "forged/internal-kind".into(),
            2 => gear.configuration.push(ConfigurationEntry {
                key: "forged-configuration".into(),
                value: ConfigurationValue::Text("not-authored".into()),
            }),
            _ => unreachable!(),
        }
        let plan = reseal_for_refusal(plan);
        if verify_plan(&plan) {
            graph_source_checks += 1;
        } else {
            graph_core_refusals += 1;
        }
        if alteration == 0 {
            assert!(
                verify_plan(&plan),
                "renamed gear is an integrity-valid Source counterexample"
            );
        }
        assert!(
            rebuild(&document, &catalog, &definition, plan).is_err(),
            "internal forgery {alteration}"
        );
    }
    assert!(
        fore_source_checks > 0,
        "exercise an integrity-valid Fore counterexample"
    );
    assert!(
        graph_source_checks > 0,
        "exercise an integrity-valid graph counterexample"
    );
    eprintln!("canonical refusal coverage: Fore Source={fore_source_checks} Core={fore_core_refusals}; graph Source={graph_source_checks} Core={graph_core_refusals}");
    let mut extra_cord = definition.internal_plan.clone();
    let gear = &extra_cord.fragments[0].placements[0];
    let cord = PlannedConnection {
        connection_id: "forged/internal-cord".into(),
        source_placement_id: gear.placement_id.clone(),
        source_port_id: gear.outputs[0].port_id.clone(),
        sink_placement_id: gear.placement_id.clone(),
        sink_port_id: gear.inputs[0].port_id.clone(),
        value_kind: gear.outputs[0].value_kind.clone(),
        resource: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: gear.outputs[0].temporal,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        selected_line: None,
        admitted_lines: vec![],
        item_capacity: 1,
        byte_capacity: gear.limits.max_queue_bytes,
    };
    extra_cord.fragments[0].connections.push(cord);
    // This particular model has25index inputs and76score outputs. Core can
    // already reject its ill-typed self-loop; the canonical graph also requires
    // zero internal cords, independently of any claimed Source identity.
    assert!(rebuild(
        &document,
        &catalog,
        &definition,
        reseal_for_refusal(extra_cord)
    )
    .is_err());
    let mut forged_definition = definition;
    forged_definition.external_capability.limits.max_queue_bytes += 1;
    assert!(matches!(
        PreparedCategoricalChannel::<()>::prepare_canonical(
            &document,
            "parser-round-numeric",
            &catalog,
            forged_definition,
            profile,
            1
        ),
        Err(CategoricalChannelRefusal::Definition)
    ));
}
