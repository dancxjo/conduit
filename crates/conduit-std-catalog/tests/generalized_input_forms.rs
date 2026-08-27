use conduit_core::{
    BootId, ConnectionBase, HostAdvertisement, HostId, HostProfileId, OfferGeneration, Quantity,
    QuantityUnit, StructuredInfoValue, StructuredInfoValueShape, StructuredSelection,
    StructuredSelector, UnmatchedVariantDisposition, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_presentation::install_geometry_catalogs;
use conduit_std_catalog::{
    deterministic_generalized_input_fixture, gamepad_state_type, generalized_input_std_offers,
    input_axis_slot_type, input_axis_slots_type, input_axis_state_type,
    input_button_transition_type, install_generalized_input_catalogs, pointer_event_type,
    touch_contacts_type, validate_normalized_axis, validate_normalized_pressure,
    GeneralizedInputRefusal, GENERALIZED_INPUT_HOST_OPERATION, MAXIMUM_INPUT_AXES,
    MAXIMUM_INPUT_BUTTONS, MAXIMUM_TOUCH_CONTACTS, NORMALIZED_BIPOLAR_AXIS_PROFILE,
};

const SOURCE: &str = include_str!("../../../examples/generalized-input.conduit");

#[test]
fn canonical_form_consumes_gamepad_button_pointer_touch_and_rotary_info() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_geometry_catalogs(&mut startup, &mut profile).unwrap();
    install_generalized_input_catalogs(&mut startup, &mut profile).unwrap();
    let parsed = parse_syntax_document(SOURCE);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "generalized-input", &profile).unwrap();
    assert_eq!(authored.expanded.gears.len(), 2);
    assert_eq!(authored.output_bindings.len(), 5);

    let host = host();
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &authored.expanded,
        &[host],
        &placements,
        &[ConnectionBase::Local],
    )
    .unwrap();
    for placement in &plan.fragments[0].placements {
        assert_eq!(
            placement.host_operations[0].contract_id.as_str(),
            GENERALIZED_INPUT_HOST_OPERATION
        );
        assert!(placement.resources.is_empty());
        assert!(placement.authority.is_empty());
    }
}

#[test]
fn structured_selection_routes_one_semantic_axis_without_new_syntax() {
    let fixture = deterministic_generalized_input_fixture().unwrap();
    let axes = select(
        &StructuredSelector::field(gamepad_state_type(), "axes").unwrap(),
        &fixture.gamepad,
    );
    let first = select(
        &StructuredSelector::index(input_axis_slots_type(), 0).unwrap(),
        &axes,
    );
    let axis = select(
        &StructuredSelector::variant(
            input_axis_slot_type(),
            "axis",
            UnmatchedVariantDisposition::Refuse,
        )
        .unwrap(),
        &first,
    );
    assert_eq!(axis.value_type(), &input_axis_state_type());
    assert_eq!(
        leaf_text(record_field(&axis, "axis_identity")),
        "axis/left-x"
    );
    assert_eq!(
        leaf_text(record_field(&axis, "range_profile")),
        NORMALIZED_BIPOLAR_AXIS_PROFILE
    );
}

#[test]
fn normalized_axes_and_pressure_refuse_hidden_device_integer_semantics() {
    assert_eq!(
        validate_normalized_axis(Quantity::new(1, QuantityUnit::Meter)),
        Err(GeneralizedInputRefusal::NonRatio)
    );
    assert_eq!(
        validate_normalized_axis(Quantity::new(1_000_001, QuantityUnit::Millionth)),
        Err(GeneralizedInputRefusal::OutsideNormalizedRange)
    );
    assert_eq!(
        validate_normalized_axis(Quantity::new(-1_000_000, QuantityUnit::Millionth)),
        Ok(())
    );
    assert_eq!(
        validate_normalized_pressure(Quantity::new(-1, QuantityUnit::Millionth)),
        Err(GeneralizedInputRefusal::OutsideNormalizedRange)
    );
}

#[test]
fn simultaneous_controls_and_pressure_evidence_are_fixed_and_inspectable() {
    let fixture = deterministic_generalized_input_fixture().unwrap();
    let axes = collection(record_field(&fixture.gamepad, "axes"));
    let buttons = collection(record_field(&fixture.gamepad, "buttons"));
    let contacts = collection(record_field(&fixture.touch, "contacts"));
    assert_eq!(axes.len(), usize::from(MAXIMUM_INPUT_AXES));
    assert_eq!(buttons.len(), usize::from(MAXIMUM_INPUT_BUTTONS));
    assert_eq!(contacts.len(), usize::from(MAXIMUM_TOUCH_CONTACTS));
    assert_eq!(variant_tag(&axes[0]), "axis");
    assert_eq!(variant_tag(&axes[2]), "unused");
    assert_eq!(variant_tag(&contacts[0]), "contact");
    assert_eq!(variant_tag(&contacts[1]), "unused");

    let pointer_pressure = record_field(&fixture.pointer, "pressure");
    assert_eq!(leaf_text(record_field(pointer_pressure, "coalesced")), "2");
    assert_eq!(leaf_text(record_field(pointer_pressure, "dropped")), "1");
    assert_eq!(
        variant_tag(record_field(pointer_pressure, "policy")),
        "coalesce_latest_state"
    );
}

#[test]
fn state_and_transition_families_remain_distinct_and_mechanism_free() {
    assert_ne!(gamepad_state_type(), input_button_transition_type());
    assert_ne!(pointer_event_type(), touch_contacts_type());
    let rendered = format!(
        "{:?}{:?}{:?}",
        gamepad_state_type(),
        pointer_event_type(),
        touch_contacts_type()
    )
    .to_ascii_lowercase();
    for forbidden in [
        "usb",
        "hid",
        "dom",
        "bluetooth",
        "winit",
        "button-code",
        "axis-code",
        "report-layout",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "input schema leaked {forbidden}"
        );
    }
}

fn select(selector: &StructuredSelector, value: &StructuredInfoValue) -> StructuredInfoValue {
    let StructuredSelection::Matched(value) = selector.select(value).unwrap() else {
        panic!("deterministic input selector must match")
    };
    value
}

fn host() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/generalized-input-proof"),
        boot_id: BootId::from("boot/generalized-input-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/generalized-input-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: generalized_input_std_offers(),
    }
}

fn record_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("expected record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}

fn collection(value: &StructuredInfoValue) -> &[StructuredInfoValue] {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("expected collection")
    };
    values
}

fn variant_tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("expected variant")
    };
    tag
}

fn leaf_text(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("expected leaf")
    };
    core::str::from_utf8(bytes).unwrap()
}
