//! Actual Source custody, transactional publication and ordinary Host Calls.
use crate::StdHost;
use conduit_core::*;
use conduit_plot::rust_binding::primitive_into_structured;
use conduit_plot::*;
use std::collections::BTreeMap;
fn profile() -> ProfileCatalog {
    let mut profile = ProfileCatalog::new();
    profile.enable_checked_native_law_expressions();
    profile
}

fn fixture() -> (
    ExpandedAuthoringPlot,
    LawExpressionContext,
    StructuredInfoType,
) {
    let source = "type Ordered = {\n hi: U8\n lo: U8\n where .lo <= .hi\n}\nplot arithmetic (\n >> value: Ordered\n result: Boolean >>\n) = (true)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let witness = checked.native_law_witness("Ordered").unwrap();
    let context =
        LawExpressionContext::from_source_witnesses(core::slice::from_ref(witness)).unwrap();
    let ty = witness.checked_type().value_type.clone();
    let authored = expand_canonical_plot_for_authoring(&checked, "arithmetic", &profile()).unwrap();
    assert!(authored.expanded.gears[0]
        .kind_id
        .as_str()
        .starts_with("conduitese/law-expression/"));
    (authored, context, ty)
}
fn frame(ty: &StructuredInfoType, lo: u8, hi: u8) -> Vec<u8> {
    let field = |name: &str, value: u8| {
        StructuredFieldValue::new(
            name,
            primitive_into_structured(
                StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
                &value,
            )
            .unwrap(),
        )
        .unwrap()
    };
    StructuredInfoValue::record(ty.clone(), vec![field("hi", hi), field("lo", lo)])
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
fn plan(authored: &ExpandedAuthoringPlot, host: &StdHost) -> Plan {
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 4096,
            },
        )
    })
    .collect();
    conduit_planner::plan_expanded_authoring_with_options(
        authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap()
}

#[test]
fn independent_source_context_installs_once_and_survives_ordinary_kernel_host_call() {
    let (authored, context, ty) = fixture();
    let mut host = StdHost::new();
    let before = host.advertisement().clone();
    assert!(host
        .prepare_expression_realizations(&authored.expanded)
        .is_err());
    assert!(host.advertisement() == &before);
    host.prepare_expression_realizations_with_law_context(&authored.expanded, &context)
        .unwrap();
    let installed = host.advertisement().clone();
    drop(context);
    host.prepare_expression_realizations(&authored.expanded)
        .unwrap();
    assert!(host.advertisement() == &installed);
    let plan = plan(&authored, &host);
    assert!(verify_plan(&plan));
    assert_eq!(
        plan.fragments[0].placements[0].implementation_id.as_str(),
        conduit_std_offers::LAW_EXPRESSION_STD_IMPLEMENTATION
    );
    let mut hosts =
        super::prepare_hosts_with_law_cache(&plan.fragments[0], Some(&host.law_preparation))
            .unwrap();
    assert_eq!(
        hosts[0]
            .as_mut()
            .unwrap()
            .execute(&frame(&ty, 1, 2))
            .unwrap(),
        &[1]
    );
    assert!(hosts[0]
        .as_mut()
        .unwrap()
        .execute(&frame(&ty, 2, 1))
        .is_err());
    // The real run below owns a fresh evaluator and crosses the generic call.
    let input = crate::ExternalForeInput {
        front_port_id: "value".into(),
        track: ConnectionTrack::Payload,
        bytes: frame(&ty, 1, 2),
    };
    let mut outputs = crate::host_execution::scalar_tape_fixture::Outputs::default();
    let mut timer = crate::host_execution::scalar_tape_fixture::NoTimer;
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[input],
            &mut outputs,
            &mut Vec::new(),
            &mut timer,
        )
        .unwrap();
    assert!(!outputs.0.is_empty());
    assert_eq!(outputs.0[0].as_slice(), &[1]);
    let kernel = report.kernel.expect("ordinary kernel receipt");
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallRequested));
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::BackCompleted));
    let mut invalid_outputs = crate::host_execution::scalar_tape_fixture::Outputs::default();
    let why = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[crate::ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: frame(&ty, 2, 1),
            }],
            &mut invalid_outputs,
            &mut Vec::new(),
            &mut timer,
        )
        .unwrap_err();
    assert!(
        why.contains("BackFailed") && why.contains("HostCallFailed"),
        "{why}"
    );
    assert!(invalid_outputs.0.is_empty());
    assert!(host.kernel_resources.is_idle());
}

#[test]
fn foreign_context_and_active_reservations_leave_custody_and_offers_unchanged() {
    let (authored, context, _) = fixture();
    let mut host = StdHost::new();
    host.prepare_expression_realizations_with_law_context(&authored.expanded, &context)
        .unwrap();
    let before = host.advertisement().clone();
    let empty = LawExpressionContext::from_source_witnesses(&[]).unwrap();
    assert!(host
        .prepare_expression_realizations_with_law_context(&authored.expanded, &empty)
        .is_err());
    assert!(host.advertisement() == &before);
    let equivalent_source =
        "# Independent module\ntype Ordered = {\n hi: U8\n lo: U8\n where .lo <= .hi\n}\n";
    let equivalent = check_syntax_document(
        &parse_syntax_document(equivalent_source),
        &StartupCatalog::new(),
    )
    .unwrap();
    let equivalent_witness = equivalent.native_law_witness("Ordered").unwrap();
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("capsule")
    };
    let material = LawExpressionMaterial::from_canonical_hex(encoded).unwrap();
    assert_eq!(
        material.roots,
        vec![equivalent_witness.material().unwrap()],
        "unrelated module text does not alter the retained declaration closure"
    );
    let equivalent_context =
        LawExpressionContext::from_source_witnesses(core::slice::from_ref(equivalent_witness))
            .unwrap();
    host.prepare_expression_realizations_with_law_context(&authored.expanded, &equivalent_context)
        .unwrap();
    assert!(host.advertisement() == &before);
    let foreign_source = equivalent_source.replace(".lo <= .hi", ".lo < .hi");
    let foreign = check_syntax_document(
        &parse_syntax_document(&foreign_source),
        &StartupCatalog::new(),
    )
    .unwrap();
    let foreign_witness = foreign.native_law_witness("Ordered").unwrap();
    assert_ne!(
        material.roots[0].claimed.identity,
        foreign_witness.checked_type().identity
    );
    let foreign_context =
        LawExpressionContext::from_source_witnesses(core::slice::from_ref(foreign_witness))
            .unwrap();
    assert!(host
        .prepare_expression_realizations_with_law_context(&authored.expanded, &foreign_context)
        .is_err());
    assert!(host.advertisement() == &before);
    let plan = plan(&authored, &host);
    let reservation = host
        .kernel_resources
        .prepare_and_reserve(&before, &plan.fragments[0])
        .unwrap();
    assert!(host
        .prepare_expression_realizations_with_law_context(&authored.expanded, &context)
        .unwrap_err()
        .contains("active reservations"));
    assert!(host.advertisement() == &before);
    host.kernel_resources.release(reservation).unwrap();
    host.prepare_expression_realizations(&authored.expanded)
        .unwrap();
}

#[test]
fn exact_plan_capsule_profile_kind_and_source_identity_are_all_required() {
    let (authored, context, _) = fixture();
    let mut host = StdHost::new();
    host.prepare_expression_realizations_with_law_context(&authored.expanded, &context)
        .unwrap();
    let plan = plan(&authored, &host);
    assert!(super::prepare_hosts(&plan.fragments[0]).is_err());
    for change in 0..5 {
        let mut fragment = plan.fragments[0].clone();
        let placement = &mut fragment.placements[0];
        match change {
            0 => placement.execution_profile_id = "std/foreign@1".into(),
            1 => placement.kind_id = kind_id("conduitese/law-expression/foreign"),
            2 => placement.artifact_id = "foreign".into(),
            3 => placement.configuration[0].value = ConfigurationValue::Text("00".into()),
            _ => fragment.expanded_plot_id = "foreign".into(),
        }
        assert!(
            super::prepare_hosts_with_law_cache(&fragment, Some(&host.law_preparation)).is_err()
        );
    }
}

#[test]
fn a_late_untrusted_law_gear_does_not_publish_an_earlier_scalar_offer_or_replace_custody() {
    let (authored, context, _) = fixture();
    let mut host = StdHost::new();
    host.prepare_expression_realizations_with_law_context(&authored.expanded, &context)
        .unwrap();
    let before = host.advertisement().clone();
    let source = "type Ordered = {\n hi: U8\n lo: U8\n where .lo <= .hi\n}\nplot arithmetic (\n >> scalar: U64\n >> record: Ordered\n wide: U64 >>\n flag: Boolean >>\n) {\n scalar >> (. + 1) >> wide\n record >> (true) >> flag\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let mixed = expand_canonical_plot_for_authoring(&checked, "arithmetic", &profile()).unwrap();
    assert!(mixed.expanded.gears[0]
        .kind_id
        .as_str()
        .starts_with("conduitese/pure-expression/"));
    assert!(mixed.expanded.gears[1]
        .kind_id
        .as_str()
        .starts_with("conduitese/law-expression/"));
    let empty = LawExpressionContext::from_source_witnesses(&[]).unwrap();
    assert!(host
        .prepare_expression_realizations_with_law_context(&mixed.expanded, &empty)
        .is_err());
    assert!(host.advertisement() == &before);
    assert!(host.kernel_resources.is_idle());
    // The retained prior capsule remains usable after the failed replacement.
    host.prepare_expression_realizations(&authored.expanded)
        .unwrap();
    assert!(host.advertisement() == &before);
}

#[test]
fn replacing_exact_expansion_custody_advances_generation_even_for_equal_offers() {
    let source = "type Ordered = {\n hi: U8\n lo: U8\n where .lo <= .hi\n}\nplot arithmetic (\n >> value: Ordered\n result: Boolean >>\n) = (true)\nplot other (\n >> value: Ordered\n result: Boolean >>\n) = (true)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let context = LawExpressionContext::from_source_witnesses(core::slice::from_ref(
        checked.native_law_witness("Ordered").unwrap(),
    ))
    .unwrap();
    let first = expand_canonical_plot_for_authoring(&checked, "arithmetic", &profile()).unwrap();
    let second = expand_canonical_plot_for_authoring(&checked, "other", &profile()).unwrap();
    let mut host = StdHost::new();
    host.prepare_expression_realizations_with_law_context(&first.expanded, &context)
        .unwrap();
    let before = host.advertisement().clone();
    let old_plan = plan(&first, &host);
    host.prepare_expression_realizations_with_law_context(&second.expanded, &context)
        .unwrap();
    assert_eq!(host.advertisement().capabilities, before.capabilities);
    assert_eq!(
        host.advertisement().offer_generation.0,
        before.offer_generation.0 + 1
    );
    assert!(super::prepare_hosts_with_law_cache(
        &old_plan.fragments[0],
        Some(&host.law_preparation)
    )
    .is_err());
    assert!(host
        .prepare_expression_realizations(&first.expanded)
        .is_err());
    host.prepare_expression_realizations(&second.expanded)
        .unwrap();
}

#[test]
fn bare_native_record_variant_and_nominal_refuse_before_scalar_selection_but_exact_custom_offers_survive(
) {
    for declaration in [
        "type Native = {\n lo: U8\n hi: U8\n where .lo <= .hi\n}\n",
        "type Native =\n known U8\n | missing\n",
        "type Native = U8 in 0..=9\n",
    ] {
        let source = format!(
            "{declaration}plot arithmetic (\n >> value: Native\n result: Boolean >>\n) = (true)\n"
        );
        let checked =
            check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
        // Default compatibility lowering deliberately retains the bare artifact.
        let authored =
            expand_canonical_plot_for_authoring(&checked, "arithmetic", &ProfileCatalog::new())
                .unwrap();
        let gear = &authored.expanded.gears[0];
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        assert!(program.requires_native_law_custody());
        let mut host = StdHost::new();
        let before = host.advertisement().clone();
        assert!(host
            .prepare_expression_realizations(&authored.expanded)
            .unwrap_err()
            .contains("native-law-witness-unavailable"));
        assert!(host.advertisement() == &before);
        let explicit =
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .unwrap();
        let mut advertisement = before;
        advertisement.capabilities.push(explicit.clone());
        let mut custom_host = StdHost::from_advertisement(advertisement).unwrap();
        let before = custom_host.advertisement().clone();
        custom_host
            .prepare_expression_realizations(&authored.expanded)
            .unwrap();
        assert!(custom_host.advertisement() == &before);
        assert!(custom_host.advertisement().capabilities.contains(&explicit));
        // This is preservation/eligibility proof; it executes no bare Native Back.
    }
}
#[test]
fn law_filter_uses_ordinary_host_calls_and_preserves_exact_optional_values() {
    let source = "type Ordered = {\n hi: U8\n lo: U8\n where .lo <= .hi\n}\nplot arithmetic (\n >> value: Ordered\n result: Ordered? >>\n) {\n value >> when(.lo < .hi) >> result\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let witness = checked.native_law_witness("Ordered").unwrap();
    let ty = witness.checked_type().value_type.clone();
    let context =
        LawExpressionContext::from_source_witnesses(core::slice::from_ref(witness)).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "arithmetic", &profile()).unwrap();
    let mut host = StdHost::new();
    host.prepare_expression_realizations_with_law_context(&authored.expanded, &context)
        .unwrap();
    let plan = plan(&authored, &host);
    assert_eq!(
        plan.fragments[0].placements[0].implementation_id.as_str(),
        conduit_std_offers::LAW_FILTER_STD_IMPLEMENTATION
    );
    let mut expected = PreparedOptionalInfoEncoder::new(ty.clone()).unwrap();
    for (lo, hi, selected) in [(1, 2, true), (2, 2, false)] {
        let input = frame(&ty, lo, hi);
        let expected = expected
            .encode(selected.then_some(input.as_slice()))
            .unwrap()
            .to_vec();
        let mut output = crate::host_execution::scalar_tape_fixture::Outputs::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[crate::ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: input,
                }],
                &mut output,
                &mut Vec::new(),
                &mut crate::host_execution::scalar_tape_fixture::NoTimer,
            )
            .unwrap();
        assert_eq!(output.0.len(), 1);
        assert_eq!(output.0[0].as_slice(), expected);
        let kernel = report.kernel.expect("ordinary filter kernel");
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    }
}
