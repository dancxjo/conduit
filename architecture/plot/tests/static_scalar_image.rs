//! CPU/static-code proof; not firmware placement, Root admission or a Play.
use conduit_core::{ConfigurationValue, PrimitiveInfoKind as K};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, generate_static_scalar_expression,
    parse_syntax_document, PortableExpressionEvaluationRefusal as R, PortableExpressionProgram,
    ProfileCatalog, ScalarTapeInstruction as I, ScalarTapeOperation as O, StartupCatalog,
    StaticScalarExpression,
};
use std::borrow::Cow;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_plot::BinaryOperator as B;
const fn instruction(kind: K, destination: usize, operation: O) -> I {
    I {
        kind,
        destination,
        operation,
    }
}
static LAZY: &[I] = &[
    instruction(K::U64, 0, O::Input),
    instruction(K::U64, 1, O::Literal(Cow::Borrowed(&[0; 8]))),
    instruction(K::Bool, 2, O::Binary(B::Equal, false, 0, 1)),
    instruction(K::U64, 3, O::BranchFalse(2, 7)),
    instruction(K::U64, 4, O::Literal(Cow::Borrowed(&[0; 8]))),
    instruction(K::U64, 3, O::Move(4)),
    instruction(K::U64, 6, O::Jump(11)),
    instruction(K::U64, 7, O::Input),
    instruction(K::U64, 8, O::Input),
    instruction(K::U64, 9, O::Binary(B::Divide, false, 7, 8)),
    instruction(K::U64, 3, O::Move(9)),
];
#[test]
fn resident_code_prepares_and_runs_without_heap_or_source_decode() {
    let (image, cost) =
        allocation_probe::observe(|| StaticScalarExpression::new(K::U64, K::U64, LAZY, 3).unwrap());
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert!(image.scratch_bytes() > 0);
    let mut output = [0xa5; 16];
    let (_, cost) = allocation_probe::observe(|| {
        for n in [0u64, 1, u64::MAX] {
            assert_eq!(
                image.evaluate_into(&n.to_le_bytes(), &mut output).unwrap(),
                8
            );
            assert_eq!(&output[..8], &u64::from(n != 0).to_le_bytes());
            assert_eq!(&output[8..], &[0xa5; 8]);
        }
        output.fill(0xa5);
        assert_eq!(
            image.evaluate_into(&[0; 7], &mut output),
            Err(R::InvalidInput)
        );
        assert_eq!(output, [0xa5; 16]);
        assert_eq!(
            image.evaluate_into(&[0; 8], &mut output[..7]),
            Err(R::InvalidProgram)
        );
        assert_eq!(output, [0xa5; 16]);
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
#[test]
fn resident_instruction_limit_is_unchanged_and_checked_before_execution() {
    static MAXIMUM: [I; 128] = [const { instruction(K::U8, 0, O::Input) }; 128];
    static OVERSIZE: [I; 129] = [const { instruction(K::U8, 0, O::Input) }; 129];
    let (_, cost) = allocation_probe::observe(|| {
        let image = StaticScalarExpression::new(K::U8, K::U8, &MAXIMUM, 0).unwrap();
        let mut output = [0];
        assert_eq!(image.evaluate_into(&[7], &mut output).unwrap(), 1);
        assert_eq!(output, [7]);
        assert!(matches!(
            StaticScalarExpression::new(K::U8, K::U8, &OVERSIZE, 0),
            Err(R::InvalidProgram)
        ));
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
#[test]
fn invalid_control_types_and_consumed_values_refuse_before_execution_without_allocation() {
    static CYCLE: &[I] = &[instruction(K::U8, 0, O::Jump(0))];
    static UNINITIALIZED: &[I] = &[
        instruction(K::U8, 0, O::Jump(1)),
        instruction(K::U8, 1, O::Move(0)),
    ];
    static ONE_BRANCH: &[I] = &[
        instruction(K::Bool, 0, O::Input),
        instruction(K::U8, 1, O::BranchFalse(0, 3)),
        instruction(K::U8, 1, O::Literal(Cow::Borrowed(&[1]))),
    ];
    static CONSUMED: &[I] = &[
        instruction(K::U8, 0, O::Input),
        instruction(K::U8, 1, O::Move(0)),
        instruction(K::U8, 2, O::Move(0)),
    ];
    static BAD_BOOL: &[I] = &[instruction(K::Bool, 0, O::Literal(Cow::Borrowed(&[2])))];
    static BAD_PROOF: &[I] = &[
        instruction(K::U8, 0, O::Input),
        instruction(K::U8, 1, O::Binary(B::Add, true, 0, 0)),
    ];
    let (_, cost) = allocation_probe::observe(|| {
        for (input, output, code, result) in [
            (K::U8, K::U8, CYCLE, 0),
            (K::U8, K::U8, UNINITIALIZED, 1),
            (K::Bool, K::U8, ONE_BRANCH, 1),
            (K::U8, K::U8, CONSUMED, 2),
            (K::Bool, K::Bool, BAD_BOOL, 0),
            (K::U8, K::U8, BAD_PROOF, 1),
        ] {
            assert!(matches!(
                StaticScalarExpression::new(input, output, code, result),
                Err(R::InvalidProgram)
            ));
        }
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
fn program(source: &str, entry: &str) -> PortableExpressionProgram {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
#[test]
fn unsupported_record_input_refuses_explicitly_without_runtime_fallback() {
    let p = program("type Envelope = {\n value: U64\n}\nplot constant (\n input: Envelope >> result: U64\n) = (7)\n", "constant");
    assert!(
        matches!(generate_static_scalar_expression(&p), Err(R::UnsupportedType(reason)) if reason == "scalar-image/fixed-input@1")
    );
}
#[test]
fn generated_resident_programs_compile_no_std_and_match_checked_source() {
    use sha2::Digest;
    use std::{fs, process::Command};
    let mut generated = String::new();
    let mut exercise = String::from("fn main() {\n");
    let mut cases = Vec::new();
    for operations in [1, 3, 8, 32] {
        cases.push(program(
            include_str!("fixtures/expression_baseline.conduit"),
            &format!("baseline-{operations}"),
        ));
        cases.push(program(
            include_str!("fixtures/expression_boolean_baseline.conduit"),
            &format!("boolean-{operations}"),
        ));
    }
    cases.push(program(
        "plot lazy (\n input: U64 >> result: U64\n) = (. == 0 ? 0 : . / .)\n",
        "lazy",
    ));
    cases.push(program(
        "plot add (\n input: I128 >> result: I128\n) = (. + 1)\n",
        "add",
    ));
    for (index, p) in cases.iter().enumerate() {
        let first = generate_static_scalar_expression(p).unwrap();
        assert_eq!(
            first,
            generate_static_scalar_expression(p).unwrap(),
            "deterministic Make output"
        );
        if index == 8 {
            assert_eq!(
                first.matches("const LITERAL_").count(),
                1,
                "repeated zero literals share one resident slice"
            );
        }
        generated.push_str(&format!("mod image_{index} {{\n{first}\n}}\n"));
        let digest = sha2::Sha256::digest(p.canonical_bytes().unwrap());
        exercise.push_str(&format!(
            "assert_eq!(image_{index}::SOURCE_PROGRAM_SHA256, {digest:?});\n"
        ));
        exercise.push_str(&format!(
            "let image = image_{index}::prepare().unwrap();\nlet mut output = [0xa5u8;16];\n"
        ));
        let inputs: Vec<Vec<u8>> = if index == 9 {
            [0i128, 1, -1, i128::MAX]
                .iter()
                .map(|n| n.to_le_bytes().to_vec())
                .collect()
        } else if index % 2 == 1 {
            vec![vec![0], vec![1], vec![2], vec![], vec![0, 1]]
        } else {
            [0u64, 1, 7, u64::MAX / 64, u64::MAX]
                .iter()
                .map(|n| n.to_le_bytes().to_vec())
                .chain([vec![0]])
                .collect()
        };
        for input in inputs {
            let expected = p.evaluate(&input);
            match expected {
                Ok(bytes) => exercise.push_str(&format!("let n = image.evaluate_into(&{input:?}, &mut output).unwrap(); assert_eq!(&output[..n], &{bytes:?});\n")),
                Err(error) => exercise.push_str(&format!("assert_eq!(image.evaluate_into(&{input:?}, &mut output), Err(conduit_plot::PortableExpressionEvaluationRefusal::{error:?}));\n")),
            }
        }
    }
    exercise.push_str("}\n");
    let directory =
        std::env::temp_dir().join(format!("conduit-static-scalar-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let dependencies = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let library = fs::read_dir(&dependencies)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("libconduit_plot-")
                && entry.path().extension().is_some_and(|ext| ext == "rlib")
        })
        .max_by_key(|entry| entry.metadata().unwrap().modified().unwrap())
        .unwrap()
        .path();
    for (name, contents, kind) in [
        ("image", format!("{generated}{exercise}"), "bin"),
        ("no_std", format!("#![no_std]\n{generated}"), "rlib"),
    ] {
        let source = directory.join(format!("{name}.rs"));
        let output = directory.join(name);
        fs::write(&source, contents).unwrap();
        let built = Command::new("rustc")
            .args(["--edition=2021", "--crate-type", kind, "-L"])
            .arg(format!("dependency={}", dependencies.display()))
            .arg("--extern")
            .arg(format!("conduit_plot={}", library.display()))
            .arg(&source)
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        if kind == "bin" {
            let ran = Command::new(output).output().unwrap();
            assert!(
                ran.status.success(),
                "{}",
                String::from_utf8_lossy(&ran.stderr)
            );
        }
    }
    fs::remove_dir_all(directory).unwrap();
}
