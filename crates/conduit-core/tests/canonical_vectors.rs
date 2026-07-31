use std::convert::Infallible;

use conduit_core::{
    CanonicalDescriptor, CanonicalError, CanonicalSink, CanonicalValue, FieldDisposition, Id,
    MapField,
};

const NODE_CONTRACT_HEX: &str = "434e4400220000000000000015636f6e647569742f6e6f64652d636f6e747261637400000000310000000000000002220000000000000002696422000000000000000e746578742f757070657263617365220000000000000005706f7274733200000000000000023100000000000000062200000000000000026964220000000000000002696e22000000000000000864656c697665727922000000000000000c66696e6974652d626174636822000000000000000870726573656e63652200000000000000087265717569726564220000000000000009646972656374696f6e220000000000000005696e70757422000000000000000a76616c75655f747970652200000000000000087374642f7465787422000000000000000b63617264696e616c69747922000000000000000b65786163746c792d6f6e6531000000000000000622000000000000000269642200000000000000036f757422000000000000000864656c697665727922000000000000000c66696e6974652d626174636822000000000000000870726573656e63652200000000000000087265717569726564220000000000000009646972656374696f6e2200000000000000066f757470757422000000000000000a76616c75655f747970652200000000000000087374642f7465787422000000000000000b63617264696e616c69747922000000000000000b6f6e652d6f722d6d6f7265";
const NODE_CONTRACT_HASH: &str =
    "sha256:435a3179acd68d169a179b9a71c07775eccc6d154b1c2ea0c81216a9128d9c18";
const SCALAR_HEX: &str = "434e440022000000000000000f636f6e647569742f6578616d706c6500000000310000000000000005220000000000000005656d707479002200000000000000056c696d697410fffffffffffffffffffffffffffffffd2200000000000000056e616d6573300000000000000002210000000000000002c3a921000000000000000365cc81220000000000000005726574727910000000000000000000000000000000042200000000000000077061796c6f61642000000000000000030001ff";
const SCALAR_HASH: &str = "sha256:34fa4e2373627df282dd65196665824cd6b1cfaa7d0fa04e211b13185b6f0f60";

struct VecSink(Vec<u8>);

impl CanonicalSink for VecSink {
    type Error = Infallible;

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

fn canonical_bytes(descriptor: &CanonicalDescriptor<'_>) -> Vec<u8> {
    let mut sink = VecSink(Vec::new());
    descriptor.write_canonical(&mut sink).unwrap();
    sink.0
}

fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(pair, 16).unwrap()
        })
        .collect()
}

#[test]
fn node_contract_matches_frozen_vector_and_normalizes_order() {
    let input_fields = [
        semantic("id", CanonicalValue::Identifier(Id("in"))),
        semantic("direction", CanonicalValue::Identifier(Id("input"))),
        semantic("value_type", CanonicalValue::Identifier(Id("std/text"))),
        semantic("presence", CanonicalValue::Identifier(Id("required"))),
        semantic("cardinality", CanonicalValue::Identifier(Id("exactly-one"))),
        semantic("delivery", CanonicalValue::Identifier(Id("finite-batch"))),
    ];
    let output_fields = [
        semantic("presence", CanonicalValue::Identifier(Id("required"))),
        semantic("id", CanonicalValue::Identifier(Id("out"))),
        semantic("value_type", CanonicalValue::Identifier(Id("std/text"))),
        semantic("delivery", CanonicalValue::Identifier(Id("finite-batch"))),
        semantic("direction", CanonicalValue::Identifier(Id("output"))),
        semantic("cardinality", CanonicalValue::Identifier(Id("one-or-more"))),
    ];
    let ports = [
        CanonicalValue::Map(&output_fields),
        CanonicalValue::Map(&input_fields),
    ];
    let enabled_default = CanonicalValue::Boolean(true);
    let fields = [
        MapField {
            name: Id("label"),
            value: CanonicalValue::Text("Uppercase"),
            disposition: FieldDisposition::Annotation,
        },
        MapField {
            name: Id("enabled"),
            value: CanonicalValue::Boolean(true),
            disposition: FieldDisposition::Defaulted(&enabled_default),
        },
        semantic("ports", CanonicalValue::Set(&ports)),
        semantic("id", CanonicalValue::Identifier(Id("text/uppercase"))),
    ];
    let descriptor = CanonicalDescriptor {
        kind: Id("conduit/node-contract"),
        schema_version: 0,
        body: CanonicalValue::Map(&fields),
    };

    let reversed_input_fields = [
        input_fields[2],
        input_fields[5],
        input_fields[4],
        input_fields[3],
        input_fields[1],
        input_fields[0],
    ];
    let reversed_output_fields = [
        output_fields[5],
        output_fields[1],
        output_fields[4],
        output_fields[3],
        output_fields[0],
        output_fields[2],
    ];
    let reversed_ports = [
        CanonicalValue::Map(&reversed_input_fields),
        CanonicalValue::Map(&reversed_output_fields),
    ];
    let reordered_fields = [
        semantic("id", CanonicalValue::Identifier(Id("text/uppercase"))),
        semantic("ports", CanonicalValue::Set(&reversed_ports)),
    ];
    let reordered = CanonicalDescriptor {
        kind: Id("conduit/node-contract"),
        schema_version: 0,
        body: CanonicalValue::Map(&reordered_fields),
    };

    let bytes = canonical_bytes(&descriptor);
    assert_eq!(bytes, decode_hex(NODE_CONTRACT_HEX));
    assert_eq!(canonical_bytes(&reordered), bytes);
    assert_eq!(
        descriptor.semantic_hash().unwrap().to_string(),
        NODE_CONTRACT_HASH
    );
    assert_eq!(
        reordered.semantic_hash().unwrap(),
        descriptor.semantic_hash().unwrap()
    );
}

#[test]
fn every_scalar_shape_matches_the_frozen_vector() {
    let names = [CanonicalValue::Text("é"), CanonicalValue::Text("e\u{301}")];
    let default_retry = CanonicalValue::Integer(3);
    let fields = [
        MapField {
            name: Id("retry"),
            value: CanonicalValue::Integer(4),
            disposition: FieldDisposition::Defaulted(&default_retry),
        },
        semantic("payload", CanonicalValue::Bytes(&[0, 1, 255])),
        semantic("names", CanonicalValue::List(&names)),
        semantic("limit", CanonicalValue::Integer(-3)),
        semantic("empty", CanonicalValue::Null),
    ];
    let descriptor = CanonicalDescriptor {
        kind: Id("conduit/example"),
        schema_version: 0,
        body: CanonicalValue::Map(&fields),
    };

    assert_eq!(canonical_bytes(&descriptor), decode_hex(SCALAR_HEX));
    assert_eq!(descriptor.semantic_hash().unwrap().to_string(), SCALAR_HASH);
}

#[test]
fn semantic_changes_change_the_hash() {
    let required = [semantic(
        "presence",
        CanonicalValue::Identifier(Id("required")),
    )];
    let optional = [semantic(
        "presence",
        CanonicalValue::Identifier(Id("optional")),
    )];
    let required = CanonicalDescriptor {
        kind: Id("conduit/port-contract"),
        schema_version: 0,
        body: CanonicalValue::Map(&required),
    };
    let optional = CanonicalDescriptor {
        kind: Id("conduit/port-contract"),
        schema_version: 0,
        body: CanonicalValue::Map(&optional),
    };

    assert_ne!(
        required.semantic_hash().unwrap(),
        optional.semantic_hash().unwrap()
    );
}

#[test]
fn ambiguous_collections_are_rejected() {
    let duplicate_fields = [
        semantic("id", CanonicalValue::Null),
        semantic("id", CanonicalValue::Boolean(false)),
    ];
    let map = CanonicalDescriptor {
        kind: Id("conduit/example"),
        schema_version: 0,
        body: CanonicalValue::Map(&duplicate_fields),
    };
    assert_eq!(map.semantic_hash(), Err(CanonicalError::DuplicateMapKey));

    let duplicate_values = [CanonicalValue::Integer(1), CanonicalValue::Integer(1)];
    let set = CanonicalDescriptor {
        kind: Id("conduit/example"),
        schema_version: 0,
        body: CanonicalValue::Set(&duplicate_values),
    };
    assert_eq!(set.semantic_hash(), Err(CanonicalError::DuplicateSetValue));
}
