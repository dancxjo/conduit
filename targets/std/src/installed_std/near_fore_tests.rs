//! Qualified Source must survive ordinary offer selection and Plan sealing.
use super::*;
fn prepared_fore(source: &str, exact: Option<bool>) -> Result<(StdHost, Plan), String> {
    prepared_fore_root(source, "accept", exact)
}
fn prepared_fore_root(
    source: &str,
    root: &str,
    exact: Option<bool>,
) -> Result<(StdHost, Plan), String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile)?;
    let checked = check_syntax_document(&parse_syntax_document(source), &startup)
        .map_err(|why| format!("checking: {why:?}"))?;
    let authored = expand_canonical_plot_for_authoring(&checked, root, &profile)
        .map_err(|why| format!("expansion: {why:?}"))?;
    let mut host = StdHost::new();
    host.prepare_expression_realizations(&authored.expanded)?;
    host.prepare_numerical_realizations(&authored.expanded)?;
    let hosts = [host.advertisement().clone()];
    let mut placements = conduit_planner::default_expanded_placements(&authored.expanded, &hosts)
        .map_err(|why| format!("placement: {why:?}"))?;
    if let Some(exact) = exact {
        let gear = authored
            .expanded
            .gears
            .iter()
            .find(|g| g.kind_id.as_str() == NUMERICAL_NEAR_KIND)
            .unwrap();
        let choice = placements.by_gear.get_mut(&gear.gear_id).unwrap();
        choice.capability_id = hosts[0]
            .capabilities
            .iter()
            .find(|o| {
                o.kind_id.as_str() == NUMERICAL_NEAR_KIND
                    && o.implementation.execution_profile_id.as_str()
                        == if exact { EXACT } else { FLOAT }
            })
            .unwrap()
            .capability_id
            .clone();
    }
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: NUMERICAL_NEAR_MAX_BYTES,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: NUMERICAL_NEAR_MAX_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(|why| format!("Plan: {why:?}"))?;
    assert!(verify_plan(&plan));
    assert_eq!(
        plan.source_document_id,
        parse_syntax_document(source).source_document_id()
    );
    Ok((host, plan))
}
#[test]
fn qualified_identity_is_eligible_for_an_ordinary_sealed_plan() {
    let source = "plot accept (\n >> value: Frequency near 0.01Hz\n result: Frequency near 0.1Hz >>\n) = .\n";
    let (_host, plan) = prepared_fore(source, None).unwrap();
    for port in &plan.fragments[0].fore_ports {
        assert!(port
            .value_contract
            .as_ref()
            .unwrap()
            .constraints
            .iter()
            .any(|c| matches!(c, ValueConstraint::NearQuantity { .. })));
    }
}

#[test]
fn unchanged_source_projects_then_executes_a_qualified_consumer_in_the_real_kernel() {
    let source = "plot accept (\n >> value: Frequency near 0.01Hz\n result: Frequency near 0.1Hz >>\n) = .\n\nplot project (\n >> value: Quantity <= 1800B\n result: Frequency near 0.1Hz >>\n) {\n boundary: math/near(limit=0.01Hz)\n consumer: accept()\n value >> boundary.value\n boundary.result >> consumer.value\n consumer.result >> result\n}\n";
    let mut work = ExactNumericWork::new(100_000_000);
    let q = RationalQuantity::new(
        ExactRational::ratio(1, 3, &mut work).unwrap(),
        Unit::resolve("Hz").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    let mut input = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(q, &mut input).unwrap();
    input.truncate(n);
    let mut previous = None;
    for exact in [false, true] {
        // The helper selects root accept; choose project while retaining the
        // exact same source document and reusable qualified consumer.
        let (mut host, plan) = prepared_fore_root(source, "project", Some(exact)).unwrap();
        assert_eq!(
            plan.source_document_id,
            parse_syntax_document(source).source_document_id()
        );
        if let Some(previous) = previous {
            assert_ne!(previous, plan.plan_id);
        }
        previous = Some(plan.plan_id.clone());
        let mut output = Outputs(Vec::with_capacity(1));
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: input.clone(),
                }],
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(output.0.len(), 1);
        let kernel = report.kernel.as_ref().unwrap();
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        let verifier =
            PreparedNearQuantityVerifier::new(&conduit_semantic_catalog::near_quantity_type())
                .unwrap();
        let limit = RationalQuantity::from_quantity(
            Quantity::parse_plot_literal("0.1Hz").unwrap(),
            &mut work,
        )
        .unwrap();
        let verified = verifier
            .verify(&output.0[0], FREQUENCY_INFO_ID, limit, None, &mut work)
            .unwrap();
        assert_eq!(verified.source(), q);
        assert_eq!(verified.require_exact_target().is_ok(), exact);
    }
}

#[test]
fn predicate_bounds_are_sealed_and_missing_or_modified_facts_refuse_installation() {
    let source = "plot accept (\n >> value: Frequency near 0.01Hz\n result: Frequency near 0.1Hz >>\n) = .\n";
    let (_, plan) = prepared_fore(source, None).unwrap();
    let fragment = &plan.fragments[0];
    let profile = conduit_std_offers::pure_expression_value_contract_bounds_profile();
    let placement = &fragment.placements[0];
    let property = placement
        .realization_properties
        .iter()
        .find(|p| p.profile() == &profile)
        .unwrap();
    let value = StructuredInfoValue::from_canonical_bytes(property.canonical_value()).unwrap();
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("bounds")
    };
    let read = |name: &str| {
        u64::from_le_bytes(
            raw(fields.iter().find(|f| f.name() == name).unwrap().value())
                .try_into()
                .unwrap(),
        )
    };
    assert_eq!(read("maximum-constraints"), 2);
    assert_eq!(
        read("maximum-numerical-validation-work"),
        2 * NEAR_VALUE_CONSTRAINT_WORK
    );
    assert!(read("retained-near-schema-bytes") > 0);
    assert!(read("canonical-contract-bytes") > read("retained-near-schema-bytes"));
    assert_eq!(read("maximum-validated-input-bytes"), 16384);
    assert!(crate::installed_std::pure_expression_back::prepare_hosts(fragment).is_ok());
    let mut missing = fragment.clone();
    missing.placements[0]
        .realization_properties
        .retain(|p| p.profile() != &profile);
    assert!(crate::installed_std::pure_expression_back::prepare_hosts(&missing).is_err());
    let mut modified = fragment.clone();
    let field = modified.placements[0]
        .realization_properties
        .iter_mut()
        .find(|p| p.profile() == &profile)
        .unwrap();
    let mut bytes = field.canonical_value().to_vec();
    *bytes.last_mut().unwrap() ^= 1;
    *field = StructuredConfigurationValue::new(profile, bytes).unwrap();
    assert!(crate::installed_std::pure_expression_back::prepare_hosts(&modified).is_err());
    let mut dropped_law = fragment.clone();
    dropped_law.placements[0]
        .semantic_contract
        .laws
        .retain(|law| !matches!(law, KindSemanticLaw::ValueContracts(_)));
    assert!(crate::installed_std::pure_expression_back::prepare_hosts(&dropped_law).is_err());
    let (_, different) = prepared_fore(&source.replace("0.1Hz", "0.2Hz"), None).unwrap();
    let mut stale_program = fragment.clone();
    stale_program.placements[0].configuration =
        different.fragments[0].placements[0].configuration.clone();
    assert!(crate::installed_std::pure_expression_back::prepare_hosts(&stale_program).is_err());
}

#[test]
fn weak_certificate_cannot_be_relabelled_as_a_tighter_qualified_result() {
    let source = "plot accept (\n >> value: Frequency near 0.01Hz\n result: Frequency near 0.000000001Hz >>\n) = .\n\nplot project (\n >> value: Quantity <= 1800B\n result: Frequency near 0.000000001Hz >>\n) {\n boundary: math/near(limit=0.01Hz)\n consumer: accept()\n value >> boundary.value\n boundary.result >> consumer.value\n consumer.result >> result\n}\n";
    let mut work = ExactNumericWork::new(100_000_000);
    let q = RationalQuantity::new(
        ExactRational::ratio(1, 3, &mut work).unwrap(),
        Unit::resolve("Hz").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    let mut input = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(q, &mut input).unwrap();
    input.truncate(n);
    for exact in [false, true] {
        let (mut host, plan) = prepared_fore_root(source, "project", Some(exact)).unwrap();
        let mut output = Outputs(Vec::with_capacity(1));
        let result = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: input.clone(),
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        );
        if exact {
            let report = result.unwrap();
            assert_eq!(output.0.len(), 1);
            assert_eq!(
                report.kernel.as_ref().unwrap().post_play_start_allocations,
                0
            );
        } else {
            assert!(output.0.is_empty());
            let error = result.unwrap_err();
            assert!(error.starts_with("installed kernel step: BackFailed(Failure { code: HostCallFailed, detail: 214 }); logical-gear=project/consumer/"), "{error}");
            assert!(
                error.contains(&format!(
                    " source-document={}",
                    plan.source_document_id.as_str()
                )),
                "{error}"
            );
            assert!(error.contains(" source-span=Some("), "{error}");
        }
    }
}

fn projected_packet(unit: &str) -> Vec<u8> {
    let mut work = ExactNumericWork::new(100_000_000);
    let q = RationalQuantity::new(
        ExactRational::ratio(1, 3, &mut work).unwrap(),
        Unit::resolve(unit).unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    let mut input = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(q, &mut input).unwrap();
    input.truncate(n);
    let (mut host, plan) = prepared(&format!("0.01{unit}"), false);
    let mut output = Outputs(Vec::with_capacity(1));
    host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "value".into(),
            track: ConnectionTrack::Payload,
            bytes: input,
        }],
        &mut output,
        &mut Vec::new(),
        &mut NoTimer,
    )
    .unwrap();
    output.0.remove(0).to_vec()
}
#[test]
fn canonical_forged_or_wrong_physical_packets_refuse_before_qualified_fore_ingress() {
    let source = "plot accept (\n >> value: Frequency near 0.01Hz\n result: Frequency near 0.1Hz >>\n) = .\n";
    let valid = projected_packet("Hz");
    let mut forged = valid.clone();
    let target = 0x3eaa_aaabu32.to_le_bytes();
    let offsets = forged
        .windows(4)
        .enumerate()
        .filter(|(_, bytes)| *bytes == target)
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1);
    forged[offsets[0]..offsets[0] + 4].copy_from_slice(&0x3eaa_aaaau32.to_le_bytes());
    // This is still canonical with the exact evidence schema. Only numerical
    // validation can detect the false target claim.
    assert!(StructuredInfoValue::from_canonical_bytes(&forged).is_ok());
    for (packet, accepts) in [
        (valid, true),
        (forged, false),
        (projected_packet("m"), false),
    ] {
        let (mut host, plan) = prepared_fore(source, None).unwrap();
        let mut output = Outputs(Vec::with_capacity(1));
        let result = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: packet,
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        );
        if accepts {
            let report = result.unwrap();
            assert_eq!(output.0.len(), 1);
            assert_eq!(
                report.kernel.as_ref().unwrap().post_play_start_allocations,
                0
            );
        } else {
            assert_eq!(
                result.unwrap_err(),
                "external Fore input 'value' violates its sealed value contract"
            );
            assert!(output.0.is_empty());
        }
    }
}

#[path = "near_fore_cost_tests.rs"]
mod cost;
