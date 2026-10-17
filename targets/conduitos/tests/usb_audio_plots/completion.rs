//! Exact class completion correlation; no native transfer is manufactured here.
use super::*;
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape as Shape,
    StructuredInfoValue as Value,
};

pub(super) fn input(
    ty: &StructuredInfoType,
    path: &str,
    index: usize,
    change: Option<(usize, &str)>,
    count: usize,
    tag: &str,
) -> Value {
    match ty.shape() {
        Shape::Record { fields, .. } => Value::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        input(
                            field.value_type(),
                            &format!("{path}/{}", field.name()),
                            index,
                            change,
                            count,
                            tag,
                        ),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        Shape::Variant { cases, .. } => {
            let selected = if path.ends_with("request") {
                if tag == "request-observe" {
                    "observe"
                } else {
                    "batch"
                }
            } else if tag == "request-observe" {
                "completed"
            } else {
                tag
            };
            let case = cases.iter().find(|case| case.tag() == selected).unwrap();
            Value::variant(
                ty.clone(),
                selected,
                input(case.payload_type(), path, index, change, count, tag),
            )
            .unwrap()
        }
        Shape::Sequence { element, .. } => {
            let length = if path.ends_with("wire") {
                if change == Some((index, "wire")) {
                    191
                } else {
                    192
                }
            } else if path.starts_with("/result") {
                count
            } else {
                if change == Some((0, "request-count")) {
                    7
                } else {
                    8
                }
            };
            Value::sequence(
                ty.clone(),
                (0..length)
                    .map(|i| input(element, path, i, change, count, tag))
                    .collect(),
            )
            .unwrap()
        }
        Shape::Leaf(leaf) => {
            let bytes = match leaf.as_str() {
                "value/empty" => vec![],
                "value/bytes" => vec![
                    0;
                    if change == Some((index, "wire")) {
                        191
                    } else {
                        192
                    }
                ],
                "value/u8" => vec![0],
                _ => {
                    let field = path.rsplit('/').next().unwrap();
                    let mut number = if field == "frame" {
                        (2045 + index as u64) % 2048
                    } else {
                        192u64
                    };
                    if path.starts_with("/result") && change == Some((index, field)) {
                        number += 1;
                    }
                    number.to_le_bytes().to_vec()
                }
            };
            Value::leaf(ty.clone(), bytes).unwrap()
        }
        _ => panic!("unexpected completion fixture shape"),
    }
}
#[test]
fn each_original_packet_frame_and_extent_is_required() {
    let p = program("usb-audio-completion-supported");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let encode = |change, count, tag| {
        input(&p.input_type, "", 0, change, count, tag)
            .canonical_bytes()
            .unwrap()
    };
    assert_eq!(
        evaluator.evaluate(&encode(None, 8, "completed")).unwrap(),
        [1]
    );
    for index in 0..8 {
        for field in ["frame", "requested", "actual"] {
            assert_eq!(
                evaluator
                    .evaluate(&encode(Some((index, field)), 8, "completed"))
                    .unwrap(),
                [0]
            );
        }
    }
    assert_eq!(
        evaluator.evaluate(&encode(None, 7, "completed")).unwrap(),
        [0]
    );
    for index in 0..8 {
        assert_eq!(
            evaluator
                .evaluate(&encode(Some((index, "wire")), 8, "completed"))
                .unwrap(),
            [0]
        );
    }
    assert_eq!(
        evaluator
            .evaluate(&encode(Some((0, "request-count")), 8, "completed"))
            .unwrap(),
        [0]
    );
    assert_eq!(
        evaluator
            .evaluate(&encode(None, 8, "request-observe"))
            .unwrap(),
        [0]
    );
    for tag in [
        "drained",
        "short",
        "missed-service",
        "underrun",
        "provider-lost",
    ] {
        assert_eq!(evaluator.evaluate(&encode(None, 8, tag)).unwrap(), [0]);
    }
}
