//! Real protocol code plus a sealed Plan fixture; not installed Host authority.
use crate::*;
use conduit_core::*;
use conduit_plot::*;
#[path = "../../../architecture/plot/tests/common/compiled_rust.rs"]
pub(super) mod compiled_rust;
pub(super) fn context() -> (CheckedSyntaxDocument, PortableExpressionProgram) {
    let types = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/i2c-types.conduit"
        )),
        &StartupCatalog::new(),
    )
    .unwrap();
    let mut catalog = StartupCatalog::new();
    for (name, path) in [
        ("I2cTransaction", "machine/i2c/transact/request"),
        ("I2cResult", "machine/i2c/transact/result"),
    ] {
        catalog
            .insert_checked_native_type_from_document(
                path,
                &types,
                &types
                    .native_types
                    .iter()
                    .find(|n| n.name == name)
                    .unwrap()
                    .identity,
            )
            .unwrap();
    }
    let d = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/ht16k33.conduit"
        )),
        &catalog,
    )
    .unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&d, "ht16k33-frame-request", &ProfileCatalog::new())
            .unwrap()
            .expanded;
    let ConfigurationValue::Text(text) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let p = PortableExpressionProgram::from_canonical_hex(text).unwrap();
    (d, p)
}
fn prepare(d: &CheckedSyntaxDocument, p: &PortableExpressionProgram) -> PreparedResidentExpression {
    PreparedResidentExpression::prepare(
        d,
        p,
        PortTemporal::Value,
        ResidentExpressionBounds {
            native: rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            maximum_occurrences: 4096,
            maximum_program_bytes: 512 * 1024,
            maximum_generated_bytes: 1024 * 1024,
            private_work_units: 1000,
        },
    )
    .unwrap()
}
fn seal(fragment: PlanFragment) -> Plan {
    seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
}
pub(super) fn plan(
    d: &CheckedSyntaxDocument,
    p: &PortableExpressionProgram,
    image: &PreparedResidentExpression,
    work: u32,
) -> Plan {
    let mut f = crate::tests::sealed_current_fragment();
    f.source_document_id = d.source_document_id.clone();
    let def = portable_expression_definition(p, PortTemporal::Value).unwrap();
    let g = &mut f.placements[1];
    g.capability_id = image.offer_description().capability_id.clone();
    g.limits = CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: p
            .maximum_prepared_input_bytes()
            .unwrap()
            .max(p.maximum_prepared_output_bytes().unwrap())
            .max(1),
    };
    g.kind_id = def.kind_id;
    g.kind_contract_revision = def.kind_contract_revision;
    g.inputs = def.inputs;
    g.outputs = def.outputs;
    g.semantic_contract = KindSemanticContract {
        configuration: def.configuration,
        laws: p.semantic_laws().unwrap(),
    };
    g.execution_profile_id = RESIDENT_EXPRESSION_PROFILE.into();
    g.implementation_id = RESIDENT_EXPRESSION_IMPLEMENTATION.into();
    g.artifact_id = image.artifact_id().clone();
    g.configuration = vec![ConfigurationEntry {
        key: "program".into(),
        value: ConfigurationValue::Text(p.canonical_hex().unwrap()),
    }];
    g.resources = vec![ResourceBinding {
        pool_id: "fixture/private-work".into(),
        class_id: COOPERATIVE_STEP_WORK_CLASS_ID.into(),
        units: work,
        protected: None,
        compute: None,
        content: None,
    }];
    g.host_calls.clear();
    g.base = None;
    g.authority.clear();
    g.pool_references.clear();
    g.realization_properties.clear();
    g.realization_characteristics.clear();
    g.terminal_transductions.clear();
    f.placements[0].limits.max_queue_bytes = image.maximum_input_bytes();
    f.placements[0].outputs[0].value_kind = f.placements[1].inputs[0].value_kind.clone();
    f.connections[0].value_kind = f.placements[1].inputs[0].value_kind.clone();
    f.connections[0].sink_port_id = f.placements[1].inputs[0].port_id.clone();
    f.connections[0].byte_capacity = image.maximum_input_bytes();
    seal(f)
}
fn generated(plan: &Plan) -> GeneratedEmbeddedPlan {
    let f = &plan.fragments[0];
    let (lowered, _) =
        conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(plan, &f.fragment_id)
            .unwrap();
    generate_embedded_plan_with_computations(
        f,
        &lowered,
        EmbeddedImageBounds::HOST_TOOLING,
        EmbeddedComputationBounds {
            maximum_program_bytes: 512 * 1024,
            maximum_total_program_bytes: 1024 * 1024,
        },
    )
    .unwrap()
}
#[test]
fn prepared_code_binds_only_its_exact_selected_source_contract_and_artifact() {
    let (d, p) = context();
    let image = prepare(&d, &p);
    assert_eq!(image.source(), prepare(&d, &p).source());
    assert_eq!(image.artifact_id(), prepare(&d, &p).artifact_id());
    let good = plan(&d, &p, &image, 1000);
    let f = &good.fragments[0];
    let placement = f.placements[1].placement_id.clone();
    assert!(image.bind(&good, &f.fragment_id, &placement).is_ok());
    for mutation in 0..7 {
        let mut f = f.clone();
        match mutation {
            0 => f.source_document_id = "other".into(),
            1 => f.placements[1].artifact_id = "other".into(),
            2 => f.placements[1].implementation_id = "other".into(),
            3 => f.placements[1].outputs[0].temporal = PortTemporal::Flow { closes: true },
            4 => f.placements[1].resources.clear(),
            5 => f.placements[1].configuration[0].value = ConfigurationValue::Text("00".into()),
            _ => f.placements[1].semantic_contract.laws.clear(),
        }
        let changed = seal(f);
        assert!(image
            .bind(&changed, &changed.fragments[0].fragment_id, &placement)
            .is_err());
    }
}
#[test]
fn exact_make_projection_pins_resident_configuration_without_carrying_program_text() {
    let (d, p) = context();
    let image = prepare(&d, &p);
    let plan = plan(&d, &p, &image, 1000);
    let f = &plan.fragments[0];
    let binding = image
        .bind(&plan, &f.fragment_id, &f.placements[1].placement_id)
        .unwrap();
    let inventory = [binding.computation_requirement()];
    let g = generated(&plan);
    let encoded = encode_assigned_resident_native_plan(
        &g,
        AssignedPlanMaxima::TINY_HOST,
        std::slice::from_ref(&binding),
    )
    .unwrap();
    let configs: Vec<_> = AssignedPlanRecords::new(&encoded)
        .unwrap()
        .map(Result::unwrap)
        .filter(|r| r.tag == ASSIGNED_CONFIGURATION)
        .collect();
    let resident = configs.iter().find(|r| r.payload[18] == 4).unwrap();
    assert_eq!(resident.payload.len(), 55);
    let required = AssignedPlanRequirements {
        host: AssignedIdentity::from_text(f.host_id.as_str()),
        boot: AssignedIdentity::from_text(f.boot_id.as_str()),
        host_calls: &[],
        resources: &[0],
        remote_bindings: &[],
    };
    let profile = AssignedComputationProfile {
        maximum_text_bytes: inventory[0].text_bytes,
        maximum_total_text_bytes: inventory[0].text_bytes,
        requirements: &inventory,
    };
    let admitted = decode_assigned_plan_with_computations(
        &encoded,
        AssignedPlanMaxima::TINY_HOST,
        required,
        profile,
    )
    .unwrap();
    assert_eq!(
        admitted.plan,
        AssignedIdentity::from_text(plan.plan_id.as_str())
    );
    let records = AssignedPlanRecords::new(&encoded)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        decode_assigned_records_with_computations(
            &encoded[..ASSIGNED_PLAN_HEADER_BYTES],
            &records,
            AssignedPlanMaxima::TINY_HOST,
            required,
            profile,
        )
        .unwrap(),
        admitted
    );
    let structural = assigned_resident_native_layout_requirements(
        &g,
        AssignedPlanMaxima::TINY_HOST,
        std::slice::from_ref(&binding),
    )
    .unwrap();
    let layout = conduit_kernel::assigned::ResidentLayout {
        records: &structural,
        counts: admitted.counts,
    };
    type Tables = conduit_kernel::assigned::AssignedKernelTables<2, 1, 4, 1, 1, 0>;
    let tables = Tables::prepare(
        &encoded,
        AssignedPlanMaxima::TINY_HOST,
        required,
        profile,
        &layout,
    )
    .unwrap();
    assert_eq!(
        tables.nodes[1].maximum_step_fuel,
        g.nodes[1].maximum_step_fuel
    );
    let borrowed = Tables::prepare_records(
        &encoded[..ASSIGNED_PLAN_HEADER_BYTES],
        &records,
        AssignedPlanMaxima::TINY_HOST,
        required,
        profile,
        &layout,
    )
    .unwrap();
    assert_eq!(borrowed.identity, tables.identity);
    let mut changed = structural;
    changed[0].payload_digest[0] ^= 1;
    assert!(Tables::prepare(
        &encoded,
        AssignedPlanMaxima::TINY_HOST,
        required,
        profile,
        &conduit_kernel::assigned::ResidentLayout {
            records: &changed,
            counts: admitted.counts
        }
    )
    .is_err());
    assert!(decode_assigned_plan_with_computations(
        &encoded,
        AssignedPlanMaxima::TINY_HOST,
        required,
        AssignedComputationProfile {
            requirements: &[],
            ..profile
        },
    )
    .is_err());
    assert!(p.canonical_hex().unwrap().len() > 55);
    eprintln!(
        "resident_make_program_text_bytes={} resident_record_bytes=55 assignment_bytes={}",
        p.canonical_hex().unwrap().len(),
        encoded.len()
    );
    for change_fuel in [false, true] {
        let mut g2 = g.clone();
        if change_fuel {
            g2.nodes[1].maximum_step_fuel -= 1;
        } else {
            g2.nodes[1].artifact_id = "other".into();
        }
        let binding = image
            .bind(&plan, &f.fragment_id, &f.placements[1].placement_id)
            .unwrap();
        assert!(encode_assigned_resident_native_plan(
            &g2,
            AssignedPlanMaxima::TINY_HOST,
            &[binding]
        )
        .is_err());
    }
}
#[test]
fn selected_factory_source_stays_within_the_preparation_bound() {
    let (d, p) = context();
    let image = prepare(&d, &p);
    let plan = plan(&d, &p, &image, 1000);
    let image = PreparedResidentExpression::prepare(
        &d,
        &p,
        PortTemporal::Value,
        ResidentExpressionBounds {
            native: rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            maximum_occurrences: 4096,
            maximum_program_bytes: 512 * 1024,
            maximum_generated_bytes: image.source().len() as u32,
            private_work_units: 1000,
        },
    )
    .unwrap();
    assert!(matches!(
        image.bind(
            &plan,
            &plan.fragments[0].fragment_id,
            &plan.fragments[0].placements[1].placement_id
        ),
        Err(ResidentExpressionRefusal::Bounds)
    ));
}
#[test]
fn emitted_factories_enforce_the_exact_sealed_work_grant() {
    let (d, p) = context();
    let image = prepare(&d, &p);
    let good = plan(&d, &p, &image, 1000);
    let weak_plan = plan(&d, &p, &image, 1);
    assert!(matches!(
        image.bind(
            &weak_plan,
            &weak_plan.fragments[0].fragment_id,
            &weak_plan.fragments[0].placements[1].placement_id
        ),
        Err(ResidentExpressionRefusal::Resource)
    ));
    let weak = PreparedResidentExpression::prepare(
        &d,
        &p,
        PortTemporal::Value,
        ResidentExpressionBounds {
            native: rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            maximum_occurrences: 4096,
            maximum_program_bytes: 512 * 1024,
            maximum_generated_bytes: 1024 * 1024,
            private_work_units: image.computation_work() - 1,
        },
    )
    .err()
    .unwrap();
    assert_eq!(
        weak,
        ResidentExpressionRefusal::PrivateWork {
            required: u32::from(image.computation_work()),
            maximum: image.computation_work() - 1,
        }
    );
    let bound = |plan: &Plan| {
        image
            .bind(
                plan,
                &plan.fragments[0].fragment_id,
                &plan.fragments[0].placements[1].placement_id,
            )
            .unwrap()
    };
    let code = format!("mod good {{\n{}\n}}", bound(&good).source());
    let probe = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../architecture/plot/tests/common/allocation_probe.rs");
    let exercise=format!("#[path={:?}]mod probe;#[global_allocator]static ALLOC:probe::Allocator=probe::Allocator;fn main(){{let((),cost)=probe::observe(||{{let back=good::prepare_selected_back().unwrap();assert!(back.required_step_fuel()<=good::SELECTED_STEP_FUEL);}});assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);}}",probe.to_str().unwrap());
    compiled_rust::verify_with_libraries(
        &code,
        &exercise,
        "make-resident-expression",
        &[
            "conduit_kernel",
            "conduit_plot",
            "conduit_core",
            "conduit_semantic_catalog",
            "conduit_plan_lowering",
        ],
    );
}

#[path = "resident_expression_tests/native_binding.rs"]
mod native_binding;

#[path = "resident_sign_projection_tests.rs"]
mod sign_projection;
