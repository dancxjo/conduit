//! Shape-specialized generic operator declarations, without network ordering.
use alloc::{format, string::String, vec, vec::Vec};
pub(crate) type Spec = (String, Vec<(String, String)>, Vec<(String, String)>);
fn vector(n: usize) -> String {
    format!("NumericF32Vector{n}")
}
fn pairs(items: Vec<(&str, String)>) -> Vec<(String, String)> {
    items.into_iter().map(|(n, t)| (n.into(), t)).collect()
}
fn op(specs: &mut Vec<Spec>, name: String, inputs: Vec<(&str, String)>, output: String) {
    specs.push((name, pairs(inputs), pairs(vec![("result", output)])));
}
pub(crate) fn fixed_signal_specs() -> Vec<Spec> {
    let mut specs = Vec::new();
    for (i, o) in [
        (80, 1),
        (328, 192),
        (192, 192),
        (192, 4),
        (272, 480),
        (160, 480),
        (240, 384),
        (128, 384),
        (208, 384),
        (160, 160),
        (128, 128),
        (688, 128),
        (128, 40),
    ] {
        let matrix = format!("NumericF32MatrixRef{i}x{o}");
        op(
            &mut specs,
            format!("numeric/linear{i}x{o}"),
            vec![("value", vector(i)), ("weights", matrix.clone())],
            vector(o),
        );
        if ![(272, 480), (160, 480), (240, 384), (128, 384), (208, 384)].contains(&(i, o)) {
            op(
                &mut specs,
                format!("numeric/dense{i}x{o}"),
                vec![
                    ("value", vector(i)),
                    ("weights", matrix),
                    ("bias", format!("NumericF32BiasRef{o}")),
                ],
                vector(o),
            );
        }
    }
    for n in [40, 128, 160, 192] {
        for operation in ["add", "multiply"] {
            op(
                &mut specs,
                format!("numeric/{operation}{n}"),
                vec![("left", vector(n)), ("right", vector(n))],
                vector(n),
            );
        }
        for operation in ["sigmoid", "complement"] {
            op(
                &mut specs,
                format!("numeric/{operation}{n}"),
                vec![("value", vector(n))],
                vector(n),
            );
        }
    }
    for n in [4, 40, 160, 192] {
        op(
            &mut specs,
            format!("numeric/tanh{n}"),
            vec![("value", vector(n))],
            vector(n),
        );
    }
    op(
        &mut specs,
        "numeric/sigmoid4".into(),
        vec![("value", vector(4))],
        vector(4),
    );
    for (i, n) in [
        (320, 80),
        (480, 160),
        (384, 128),
        (44, 40),
        (256, 216),
        (256, 40),
        (4, 1),
    ] {
        op(
            &mut specs,
            format!("numeric/slice{i}x{n}"),
            vec![("value", vector(i)), ("start", "U16".into())],
            vector(n),
        );
    }
    for (i, j) in [
        (80, 44),
        (124, 40),
        (164, 164),
        (192, 40),
        (232, 40),
        (160, 40),
        (200, 40),
        (128, 40),
        (168, 40),
        (160, 128),
        (288, 128),
        (416, 192),
        (608, 40),
        (648, 40),
        (216, 40),
    ] {
        op(
            &mut specs,
            format!("numeric/concatenate{i}x{j}"),
            vec![("left", vector(i)), ("right", vector(j))],
            vector(i + j),
        );
    }
    for n in [40, 44] {
        op(
            &mut specs,
            format!("numeric/scale{n}"),
            vec![("value", vector(n)), ("scale", vector(1))],
            vector(n),
        );
        op(
            &mut specs,
            format!("numeric/clamp{n}"),
            vec![
                ("value", vector(n)),
                ("minimum", vector(1)),
                ("maximum", vector(1)),
            ],
            vector(n),
        );
    }
    op(
        &mut specs,
        "numeric/exp1".into(),
        vec![("value", vector(1))],
        vector(1),
    );
    op(
        &mut specs,
        "numeric/reciprocal-offset1".into(),
        vec![("value", vector(1)), ("offset", vector(1))],
        vector(1),
    );
    op(
        &mut specs,
        "numeric/gather256x44".into(),
        vec![
            ("value", vector(256)),
            ("indices", "NumericU16Indices44".into()),
        ],
        vector(44),
    );
    op(
        &mut specs,
        "numeric/one-pole40".into(),
        vec![
            ("value", vector(40)),
            ("coefficient", vector(1)),
            ("prior", vector(1)),
        ],
        "NumericOnePole40Result".into(),
    );
    specs
}
