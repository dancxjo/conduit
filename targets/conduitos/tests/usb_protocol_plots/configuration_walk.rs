//! Complete finite descriptor walking uses Source state, not a Rust parser.
use super::program_from;
use conduit_core::{
    StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
    validate_canonical_structured_value,
};
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};

const SOURCE: &str = include_str!("../../plots/usb/configuration-walk.conduit");

fn input(program: &PortableExpressionProgram, wire: &[u8], actual: u64) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("frame")
    };
    let wire_type = fields
        .iter()
        .find(|f| f.name() == "wire")
        .unwrap()
        .value_type();
    let StructuredInfoTypeShape::Sequence { element, .. } = wire_type.shape() else {
        panic!("wire")
    };
    let bytes = StructuredInfoValue::sequence(
        wire_type.clone(),
        wire.iter()
            .map(|byte| StructuredInfoValue::leaf(element.clone(), vec![*byte]).unwrap())
            .collect(),
    )
    .unwrap();
    let count_type = fields
        .iter()
        .find(|f| f.name() == "actual")
        .unwrap()
        .value_type();
    StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new("wire", bytes).unwrap(),
            StructuredFieldValue::new(
                "actual",
                StructuredInfoValue::leaf(count_type.clone(), actual.to_le_bytes().to_vec())
                    .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

struct Walker {
    programs: [PortableExpressionProgram; 11],
    prepared: [PreparedPortableExpressionEvaluator; 11],
}
impl Walker {
    fn new() -> Self {
        let programs = [
            "usb-configuration-walk-start",
            "usb-configuration-walk-geometry",
            "usb-configuration-walk-interface",
            "usb-configuration-walk-endpoint",
            "usb-configuration-walk-default0",
            "usb-configuration-walk-default1",
            "usb-configuration-walk-default2",
            "usb-configuration-walk-default3",
            "usb-configuration-walk-unique-endpoints",
            "usb-configuration-walk-counts",
            "usb-configuration-walk-finish",
        ]
        .map(|entry| program_from(SOURCE, entry));
        let prepared = programs
            .each_ref()
            .map(|program| PreparedPortableExpressionEvaluator::new(program).unwrap());
        for evaluator in &prepared {
            assert!(
                evaluator.output_capacity() <= 4096,
                "existing native Fore budget: {}",
                evaluator.output_capacity()
            );
        }
        Self { programs, prepared }
    }
    fn run(&mut self, wire: &[u8], actual: u64, expected: &str) -> Vec<u8> {
        let mut value = input(&self.programs[0], wire, actual);
        for index in [0]
            .into_iter()
            .chain([1, 2, 3].into_iter().cycle().take(48))
            .chain([4, 5, 6, 7, 8, 9, 10])
        {
            let ordinary = self.programs[index].evaluate(&value).unwrap();
            let output = self.prepared[index].evaluate(&value).unwrap().to_vec();
            assert_eq!(output, ordinary);
            value = output;
        }
        assert!(
            validate_canonical_structured_value(&value)
                .unwrap()
                .variant_payload(expected)
                .unwrap()
                .is_some(),
            "expected {expected}"
        );
        value
    }
}
fn run(wire: &[u8], actual: u64, expected: &str) -> Vec<u8> {
    Walker::new().run(wire, actual, expected)
}
fn configuration(records: &[u8], interfaces: u8) -> Vec<u8> {
    let total = (9 + records.len()) as u16;
    let mut bytes = vec![
        9,
        2,
        total as u8,
        (total / 256) as u8,
        interfaces,
        1,
        0,
        128,
        50,
    ];
    bytes.extend_from_slice(records);
    bytes
}

#[test]
fn complete_keyboard_configuration_preserves_endpoint_parent_and_wire_fields() {
    let bytes = configuration(&[9, 4, 3, 0, 1, 3, 1, 1, 0, 7, 5, 129, 3, 8, 0, 10], 1);
    let output = run(&bytes, bytes.len() as u64, "configuration");
    let topology = validate_canonical_structured_value(&output)
        .unwrap()
        .variant_payload("configuration")
        .unwrap()
        .unwrap();
    assert_eq!(
        topology
            .record_field("interface_count")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        1_u64.to_le_bytes()
    );
    let endpoint = topology
        .record_field("endpoints")
        .unwrap()
        .unwrap()
        .collection_index(0)
        .unwrap()
        .unwrap();
    assert_eq!(
        endpoint
            .record_field("address")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u8")
            .unwrap(),
        [129]
    );
    assert_eq!(
        endpoint
            .record_field("interface_slot")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/u64")
            .unwrap(),
        0_u64.to_le_bytes()
    );
}

#[test]
fn alternate_settings_count_distinct_interface_numbers_and_allow_zero_endpoints() {
    let bytes = configuration(
        &[
            9, 4, 0, 0, 0, 1, 2, 0, 0, 9, 4, 0, 1, 1, 1, 2, 0, 0, 9, 5, 1, 1, 192, 0, 1, 0, 0,
        ],
        1,
    );
    run(&bytes, bytes.len() as u64, "configuration");
}

#[test]
fn actual_extent_and_descriptor_geometry_refuse_padding_and_malformed_records() {
    let mut walker = Walker::new();
    let valid = configuration(&[9, 4, 0, 0, 1, 3, 1, 1, 0, 7, 5, 129, 3, 8, 0, 10], 1);
    for actual in 0..valid.len() {
        walker.run(&valid[..actual], actual as u64, "short");
        walker.run(&valid, actual as u64, "malformed");
    }
    walker.run(&valid, 257, "oversized");
    let mut cases = Vec::new();
    for (offset, value) in [
        (0, 8),
        (1, 4),
        (2, 8),
        (4, 0),
        (5, 0),
        (7, 0),
        (9, 0),
        (9, 1),
        (9, 8),
        (9, 255),
        (18, 6),
        (20, 128),
        (20, 145),
        (22, 0),
    ] {
        let mut wire = valid.clone();
        wire[offset] = value;
        cases.push(wire);
    }
    cases.push(configuration(&[7, 5, 129, 3, 8, 0, 10], 1));
    cases.push(configuration(
        &[
            9, 4, 0, 0, 2, 3, 1, 1, 0, 7, 5, 129, 3, 8, 0, 10, 7, 5, 129, 3, 8, 0, 10,
        ],
        1,
    ));
    cases.push(configuration(&[9, 4, 0, 0, 1, 3, 1, 1, 0], 1));
    cases.push(configuration(&[9, 4, 0, 1, 0, 1, 2, 0, 0], 1));
    cases.push(configuration(
        &[9, 4, 0, 0, 0, 1, 2, 0, 0, 9, 4, 0, 0, 0, 1, 2, 0, 0],
        1,
    ));
    cases.push(configuration(&[9, 4, 0, 0, 0, 1, 2, 0, 0], 2));
    cases.push(configuration(&[9, 4, 0, 0, 0, 1, 2, 0, 0, 2], 1));
    cases.push(configuration(&[9, 4, 0, 0, 0, 1, 2, 0, 0, 2, 2], 1));
    for wire in cases {
        walker.run(&wire, wire.len() as u64, "malformed");
    }
}

#[test]
fn finite_record_budget_counts_unknown_class_descriptors_and_accepts_exact_boundary() {
    let mut walker = Walker::new();
    for count in [15, 16] {
        let mut records = vec![9, 4, 0, 0, 0, 1, 2, 0, 0];
        for _ in 0..count {
            records.extend_from_slice(&[2, 36]);
        }
        let wire = configuration(&records, 1);
        walker.run(
            &wire,
            wire.len() as u64,
            if count == 15 {
                "configuration"
            } else {
                "oversized"
            },
        );
    }
    let mut records = Vec::new();
    for interface in 0..5 {
        records.extend_from_slice(&[9, 4, interface, 0, 0, 1, 2, 0, 0]);
    }
    let wire = configuration(&records, 5);
    walker.run(&wire, wire.len() as u64, "oversized");
}

#[test]
fn configuration_walk_state_fits_existing_native_fore_budget() {
    let program = program_from(SOURCE, "usb-configuration-walk-start");
    let prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert!(
        prepared.output_capacity() <= 4096,
        "state bound {}",
        prepared.output_capacity()
    );
}

#[test]
fn maximum_frame_and_endpoint_capacity_preserve_actual_source_octets() {
    let mut walker = Walker::new();
    let mut records = vec![9, 4, 7, 0, 8, 3, 1, 1, 0];
    for address in 1..=8 {
        records.extend_from_slice(&[7, 5, address, 0x29, 0, 0x14, 4]);
    }
    let remaining = 256 - 9 - records.len();
    records.extend_from_slice(&[remaining as u8, 36]);
    records.resize(256 - 9, 0);
    let wire = configuration(&records, 1);
    let bytes = walker.run(&wire, 256, "configuration");
    let topology = validate_canonical_structured_value(&bytes)
        .unwrap()
        .variant_payload("configuration")
        .unwrap()
        .unwrap();
    for index in 0..8 {
        let endpoint = topology
            .record_field("endpoints")
            .unwrap()
            .unwrap()
            .collection_index(index)
            .unwrap()
            .unwrap();
        assert_eq!(
            endpoint
                .record_field("address")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u8")
                .unwrap(),
            [(index + 1) as u8]
        );
        assert_eq!(
            endpoint
                .record_field("attributes")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u8")
                .unwrap(),
            [0x29]
        );
        assert_eq!(
            endpoint
                .record_field("packet_field")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            0x1400_u64.to_le_bytes()
        );
    }
    // The address may repeat on a distinct alternate setting; the parent differs.
    let wire = configuration(
        &[
            9, 4, 0, 0, 1, 3, 1, 1, 0, 7, 5, 129, 3, 8, 0, 10, 9, 4, 0, 1, 1, 3, 1, 1, 0, 7, 5,
            129, 3, 8, 0, 10,
        ],
        1,
    );
    walker.run(&wire, wire.len() as u64, "configuration");
}

#[path = "../../../../architecture/plot/tests/prepared_structured_payload/allocation.rs"]
mod allocation;

#[test]
fn complete_walk_reuses_prepared_storage_without_allocating() {
    let mut walker = Walker::new();
    let valid = configuration(&[9, 4, 0, 0, 1, 3, 1, 1, 0, 7, 5, 129, 3, 8, 0, 10], 1);
    let invalid = configuration(&[9, 4, 0, 0, 1, 3, 1, 1, 0], 1);
    let inputs = [
        input(&walker.programs[0], &valid, valid.len() as u64),
        input(&walker.programs[0], &invalid, invalid.len() as u64),
    ];
    let expected = [
        walker.run(&valid, valid.len() as u64, "configuration"),
        walker.run(&invalid, invalid.len() as u64, "malformed"),
    ];
    let mut value = Vec::with_capacity(4096);
    let mut scratch = Vec::with_capacity(4096);
    assert_eq!(
        allocation::allocations(|| {
            for _ in 0..256 {
                for (initial, expected) in inputs.iter().zip(&expected) {
                    value.clear();
                    value.extend_from_slice(initial);
                    for index in [0]
                        .into_iter()
                        .chain([1, 2, 3].into_iter().cycle().take(48))
                        .chain([4, 5, 6, 7, 8, 9, 10])
                    {
                        scratch.clear();
                        scratch.extend_from_slice(walker.prepared[index].evaluate(&value).unwrap());
                        std::mem::swap(&mut value, &mut scratch);
                    }
                    assert_eq!(&value, expected);
                }
            }
        }),
        0
    );
    assert_eq!(value.capacity(), 4096);
    assert_eq!(scratch.capacity(), 4096);
}

#[test]
fn complete_checked_topology_stays_inside_the_existing_kernel_node_profile() {
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(SOURCE),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "usb-configuration-walk",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    assert_eq!(expanded.gears.len(), 56);
}
