use super::*;

const SOURCE: &str = concat!(
    include_str!("../../../../../plots/device-protocols/bme280-lifecycle.conduit"),
    "\nplot retained (\n >> query: BmeProtocolBegin...|\n >> next: BmeProtocolState...| <= 4096B\n current: $BmeProtocolState <= 4096B >>\n) {\n cell: state/seeded/finite\n query >> bme280-protocol-initialize() >> cell.seed\n next >> cell.next\n cell.current >> current\n}\n"
);

#[test]
fn actual_bme_source_initializer_seeds_exact_native_state_through_the_kernel() {
    let contract = crate::i2c_base::contract::I2cContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    // Derive the cell's exact schema from the reviewed lifecycle definitions.
    let lifecycle = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../../plots/device-protocols/bme280-lifecycle.conduit"
        )),
        &startup,
    )
    .unwrap();
    let ty = |name| {
        &lifecycle
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
    };
    let schema = ty("BmeProtocolState");
    let value =
        CheckedValueContract::new(schema.profile().unwrap().value_kind().clone(), 4096, vec![])
            .unwrap();
    let (plan, factory, offer) =
        planned_source(startup, profile, &value, schema, SOURCE, "retained");
    factory.validate_plan(&plan).unwrap();
    assert_eq!(plan.fragments[0].placements.len(), 2);
    let fragment = &plan.fragments[0];
    let initializer = fragment
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == crate::expression_host_call::IMPLEMENTATION)
        .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let mut expression = crate::expression_host_call::ExpressionHostCall::prepare(
        fragment,
        &lowered,
        &active,
        &initializer.placement_id,
    )
    .unwrap();
    let host = PreparationHostIdentity {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
    };
    let mut play = kernel(plan, factory, offer);
    let begin = ty("BmeProtocolBegin");
    let StructuredInfoTypeShape::Record { fields, .. } = begin.shape() else {
        panic!("begin")
    };
    let query = StructuredInfoValue::record(
        begin.clone(),
        vec![
            StructuredFieldValue::new(
                "address",
                StructuredInfoValue::leaf(fields[0].value_type().clone(), vec![118]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let payload = ValuePayload {
        value_kind: begin.profile().unwrap().value_kind().clone(),
        encoded: query.canonical_bytes().unwrap(),
    };
    let mut output = ValuePayload {
        value_kind: value.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    play.start().unwrap();
    play.admit_input(&port_id("query"), 0, &payload).unwrap();
    play.close_input(&port_id("query")).unwrap();
    for _ in 0..32 {
        play.step().unwrap();
        if let Some(request) = play.next_host_request() {
            let admitted = play.admit_host_request(&request, &host, &[], &[]).unwrap();
            let view = play.admitted_host_request_view(&admitted).unwrap();
            let output = expression
                .invoke(
                    view.request.node,
                    view.request.call,
                    view.request.request,
                    play.host_request_input(&admitted).unwrap(),
                )
                .unwrap();
            play.complete_host_call_bytes(&admitted, output).unwrap();
        }
    }
    assert_eq!(
        play.output_into(&port_id("current"), &mut output).unwrap(),
        Some(0)
    );
    let initialized = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
    assert_eq!(initialized.value_type(), schema);
    let StructuredInfoValueShape::Record(fields) = initialized.shape() else {
        panic!("state")
    };
    let field = |name| {
        fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value()
    };
    assert!(
        matches!(field("address").shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == &[118])
    );
    assert!(
        matches!(field("phase").shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == &[0])
    );
    play.complete_output(&port_id("current"), 0).unwrap();
    play.admit_input(&port_id("next"), 0, &output).unwrap();
    for _ in 0..16 {
        play.step().unwrap();
    }
    let mut replacement = ValuePayload {
        value_kind: value.value_kind,
        encoded: Vec::with_capacity(4096),
    };
    assert_eq!(
        play.output_into(&port_id("current"), &mut replacement)
            .unwrap(),
        Some(1)
    );
    assert_eq!(replacement, output);
    play.complete_output(&port_id("current"), 1).unwrap();
    play.close_input(&port_id("next")).unwrap();
    let mut status = KernelCompositeStatus::Active;
    for _ in 0..16 {
        status = play.step().unwrap();
    }
    assert_eq!(status, KernelCompositeStatus::Complete);
}
