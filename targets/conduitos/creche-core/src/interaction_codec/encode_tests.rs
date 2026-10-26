use super::*;
use ciborium::value::Value;
fn encoded<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, ()> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(&Ref(value), &mut bytes).map_err(|_| ())?;
    Ok(bytes)
}
fn value<T: Serialize + ?Sized>(item: &T) -> Value {
    ciborium::de::from_reader(encoded(item).unwrap().as_slice()).unwrap()
}
#[test]
fn packs_genuine_bytes_and_preserves_wider_signed_empty_and_mixed_sequences() {
    assert_eq!(
        value(&[0u8, 23, 24, 255]),
        Value::Bytes(alloc::vec![0, 23, 24, 255])
    );
    assert!(matches!(value(&[0u16, 23, 24, 255, 256]), Value::Array(_)));
    assert!(matches!(value(&[0i8, 23, -1]), Value::Array(_)));
    assert_eq!(value(&[] as &[u8]), Value::Array(Vec::new()));
    let Value::Array(mixed) = value(&(7u8, 255u16, alloc::vec![1u8, 2])) else {
        panic!("mixed tuple must stay array")
    };
    assert_eq!(mixed.len(), 3);
    assert_eq!(mixed[0], Value::Integer(7.into()));
    assert_eq!(mixed[1], Value::Integer(255.into()));
    assert_eq!(mixed[2], Value::Bytes(alloc::vec![1, 2]));
}
#[test]
fn nested_maps_options_structs_and_variants_recurse_without_changing_named_presence() {
    #[derive(Serialize)]
    struct Record {
        bytes: Vec<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        absent: Option<u8>,
    }
    #[derive(Serialize)]
    enum Example {
        New(Record),
        Tuple(u8, Vec<u8>),
        Fields { bytes: Vec<u8> },
    }
    for record in [
        Example::New(Record {
            bytes: alloc::vec![3, 4],
            absent: None,
        }),
        Example::Tuple(9, alloc::vec![3, 4]),
        Example::Fields {
            bytes: alloc::vec![3, 4],
        },
    ] {
        let Value::Map(fields) = value(&Some(record)) else {
            panic!("variant must remain named map")
        };
        assert_eq!(fields.len(), 1);
    }
    let Value::Map(fields) = value(&Record {
        bytes: alloc::vec![3, 4],
        absent: None,
    }) else {
        panic!("named fields required")
    };
    assert_eq!(
        fields,
        alloc::vec![(Value::Text("bytes".into()), Value::Bytes(alloc::vec![3, 4]))]
    );
}
#[test]
fn native_bytes_and_bignum_byte_encodings_refuse_recursively() {
    struct Native;
    impl Serialize for Native {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_bytes(&[1, 2])
        }
    }
    assert!(encoded(&Native).is_err());
    assert!(encoded(&Some(alloc::vec![Native])).is_err());
    assert!(encoded(&(u128::from(u64::MAX) + 1)).is_err());
    assert!(encoded(&(-(i128::from(u64::MAX)) - 2)).is_err());
    for integer in [i128::from(u64::MAX), -i128::from(u64::MAX) - 1] {
        let mut ordinary = Vec::new();
        ciborium::ser::into_writer(&integer, &mut ordinary).unwrap();
        assert_eq!(encoded(&integer).unwrap(), ordinary);
    }
}

#[test]
fn packed_candidates_refuse_wrong_declared_lengths_including_empty() {
    struct Miscount {
        kind: u8,
        declared: usize,
        actual: usize,
    }
    impl Serialize for Miscount {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            match self.kind {
                0 => {
                    let mut seq = serializer.serialize_seq(Some(self.declared))?;
                    for _ in 0..self.actual {
                        seq.serialize_element(&7u8)?;
                    }
                    seq.end()
                }
                1 => {
                    let mut seq = serializer.serialize_tuple(self.declared)?;
                    for _ in 0..self.actual {
                        seq.serialize_element(&7u8)?;
                    }
                    seq.end()
                }
                _ => {
                    let mut seq = serializer.serialize_tuple_struct("Fixture", self.declared)?;
                    for _ in 0..self.actual {
                        seq.serialize_field(&7u8)?;
                    }
                    seq.end()
                }
            }
        }
    }
    for kind in 0..3 {
        for (declared, actual) in [(1, 0), (0, 1), (1, 2), (2, 1)] {
            assert!(
                encoded(&Miscount {
                    kind,
                    declared,
                    actual
                })
                .is_err()
            );
        }
        assert!(
            encoded(&Miscount {
                kind,
                declared: 0,
                actual: 0
            })
            .is_ok()
        );
        assert_eq!(
            value(&Miscount {
                kind,
                declared: 2,
                actual: 2
            }),
            Value::Bytes(alloc::vec![7, 7])
        );
    }
}
