//! Native-image CPU proof, not installed authority or physical device admission.
use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
#[path = "common/compiled_rust.rs"]
mod compiled_rust;
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: 4096,
        scopes: 64,
        invariants: 64,
        contracts: 64,
    }
}
fn checked(source: &str) -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap()
}
fn native<'a>(d: &'a CheckedSyntaxDocument, name: &str) -> &'a CheckedNativeType {
    d.native_types.iter().find(|n| n.name == name).unwrap()
}
fn reference(
    d: &CheckedSyntaxDocument,
    identity: &KindId,
    bytes: &[u8],
    occurrences: usize,
) -> Result<(), PreparedNativeRefusal> {
    let mut admitted =
        PreparedNativeAdmission::new_with_occurrence_limit(d, identity, bounds(), occurrences)?;
    admitted.begin(bytes)?;
    loop {
        if admitted.step()? == PreparedNativeProgress::Admitted {
            return Ok(());
        }
    }
}
fn corpus(
    d: &CheckedSyntaxDocument,
    names: &[&str],
    values: impl Fn(&str) -> Vec<Vec<u8>>,
    occurrences: usize,
) {
    let mut generated = String::new();
    let allocator =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/common/allocation_probe.rs");
    let mut exercise = format!("#[path={:?}] mod probe;\n#[global_allocator] static ALLOC: probe::Allocator = probe::Allocator;\nfn main() {{\n", allocator.to_str().unwrap());
    for (index, name) in names.iter().enumerate() {
        let identity = &native(d, name).identity;
        let image = generate_resident_native_validator(d, identity, bounds(), occurrences)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(
            image,
            generate_resident_native_validator(d, identity, bounds(), occurrences).unwrap()
        );
        generated.push_str(&format!("mod image_{index} {{\n{image}\n}}\n"));
        exercise.push_str(&format!("assert_eq!(image_{index}::NATIVE_ID, {:?});\nlet (validator,cost)=probe::observe(||image_{index}::prepare().unwrap()); assert_eq!(cost.allocations,0); assert_eq!(cost.reallocations,0); assert!(image_{index}::RUNTIME_STATE_BYTES < 1024);\n", identity.as_str()));
        for input in values(name) {
            let expected = format!("{:?}", reference(d, identity, &input, occurrences));
            exercise.push_str(&format!("let (actual,cost)=probe::observe(||validator.validate(&{input:?})); assert_eq!(cost.allocations,0);assert_eq!(cost.reallocations,0);assert_eq!(format!(\"{{:?}}\",actual),{expected:?});\n"));
        }
    }
    exercise.push_str("}\n");
    compiled_rust::verify(
        &generated,
        &exercise,
        &format!("native-image-{occurrences}"),
    );
}
#[test]
fn retained_native_owners_cannot_be_replaced_by_inspection_metadata() {
    let d = checked("type Address = U8 in 8..=119\n");
    let id = native(&d, "Address").identity.clone();
    let mut changed = d.clone();
    changed.native_types[0].value_contracts.clear();
    assert_eq!(
        generate_resident_native_validator(&changed, &id, bounds(), 4096),
        Err(ResidentNativeGenerationRefusal::Admission(
            PreparedNativeRefusal::ConflictingNativeMetadata
        ))
    );
    let mut hidden = d.clone();
    hidden.native_types.clear();
    assert!(generate_resident_native_validator(&hidden, &id, bounds(), 4096).is_ok());
    let fake = checked("type Forged = U8 in 1..=2\n")
        .native_types
        .remove(0);
    let fake_id = fake.identity.clone();
    hidden.native_types.push(fake);
    assert_eq!(
        generate_resident_native_validator(&hidden, &fake_id, bounds(), 4096),
        Err(ResidentNativeGenerationRefusal::Admission(
            PreparedNativeRefusal::MissingNativeMetadata
        ))
    );
}
#[test]
fn unsupported_witnesses_and_insufficient_capacity_refuse_at_make() {
    let d = checked("type Law = {\n value: U8\n where .value < 8\n}\n");
    assert!(matches!(
        generate_resident_native_validator(&d, &native(&d, "Law").identity, bounds(), 4096),
        Err(ResidentNativeGenerationRefusal::UnsupportedInvariant { .. })
    ));
    let d = checked("type Label = Text <= 8B not in [\"\"]\n");
    assert!(matches!(
        generate_resident_native_validator(&d, &native(&d, "Label").identity, bounds(), 4096),
        Err(ResidentNativeGenerationRefusal::UnsupportedContract { .. })
    ));
    let d = checked("type Address = U8 in 8..=119\n");
    let mut short = bounds();
    short.contracts = 0;
    assert_eq!(
        generate_resident_native_validator(&d, &native(&d, "Address").identity, short, 4096),
        Err(ResidentNativeGenerationRefusal::Admission(
            PreparedNativeRefusal::Bounds
        ))
    );
}
#[test]
fn compiled_scalar_native_ranges_preserve_refusals_reuse_and_combined_occurrence_pressure() {
    let d = checked("type Address = U8 in 8..=119\n");
    let ty = native(&d, "Address").value_type.clone();
    let representation = match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => representation.clone(),
        _ => panic!("nominal"),
    };
    let values = || {
        [0u8, 7, 8, 112, 119, 120, 255]
            .into_iter()
            .map(|n| {
                StructuredInfoValue::nominal(
                    ty.clone(),
                    StructuredInfoValue::leaf(representation.clone(), vec![n]).unwrap(),
                )
                .unwrap()
                .canonical_bytes()
                .unwrap()
            })
            .chain([vec![]])
            .collect::<Vec<_>>()
    };
    for limit in [1, 4096] {
        corpus(&d, &["Address"], |_| values(), limit);
    }
}

fn ht_document() -> CheckedSyntaxDocument {
    let types = checked(include_str!(
        "../../../plots/device-protocols/i2c-types.conduit"
    ));
    let mut catalog = StartupCatalog::new();
    for (name, path) in [
        ("I2cTransaction", "machine/i2c/transact/request"),
        ("I2cResult", "machine/i2c/transact/result"),
    ] {
        catalog
            .insert_checked_native_type_from_document(path, &types, &native(&types, name).identity)
            .unwrap();
    }
    check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/ht16k33.conduit"
        )),
        &catalog,
    )
    .unwrap()
}
fn sample(
    ty: &StructuredInfoType,
    address: u8,
    preferred: &str,
    field: &str,
) -> StructuredInfoValue {
    use StructuredInfoTypeShape as S;
    match ty.shape() {
        S::Leaf(kind) => {
            let bytes = match kind.as_str() {
                "value/empty" => vec![],
                "value/bool" => vec![0],
                "value/u8" => vec![if field == "address" { address } else { 0 }],
                other => panic!("unsupported fixture leaf {other}"),
            };
            StructuredInfoValue::leaf(ty.clone(), bytes).unwrap()
        }
        S::Nominal { representation, .. } => StructuredInfoValue::nominal(
            ty.clone(),
            sample(representation, address, preferred, field),
        )
        .unwrap(),
        S::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|f| {
                    StructuredFieldValue::new(
                        f.name(),
                        sample(f.value_type(), address, preferred, f.name()),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        S::Variant { cases, .. } => {
            let case = [preferred, "transact", "bus", "completed", "none"]
                .iter()
                .find_map(|name| cases.iter().find(|case| case.tag() == *name))
                .unwrap_or(&cases[0]);
            StructuredInfoValue::variant(
                ty.clone(),
                case.tag(),
                sample(case.payload_type(), address, preferred, field),
            )
            .unwrap()
        }
        S::Collection { element, length } => StructuredInfoValue::collection(
            ty.clone(),
            (0..length)
                .map(|_| sample(element, address, preferred, field))
                .collect(),
        )
        .unwrap(),
        S::Sequence {
            element,
            minimum_items,
            ..
        } => StructuredInfoValue::sequence(
            ty.clone(),
            (0..minimum_items)
                .map(|_| sample(element, address, preferred, field))
                .collect(),
        )
        .unwrap(),
    }
}
#[test]
fn real_ht16k33_native_frames_compile_and_match_authoritative_validation() {
    let d = ht_document();
    let names = [
        "Ht16k33Configuration",
        "Ht16k33State",
        "Ht16k33Action",
        "Ht16k33Frame",
        "Ht16k33Failure",
    ];
    corpus(
        &d,
        &names,
        |name| {
            let ty = &native(&d, name).value_type;
            let mut values = Vec::new();
            for address in [0, 7, 8, 112, 119, 120, 255] {
                for case in ["transact", "acknowledged", "refused", "none"] {
                    values.push(sample(ty, address, case, "").canonical_bytes().unwrap());
                }
            }
            let mut trailing = values[0].clone();
            trailing.push(0);
            values.push(trailing);
            values.push(vec![]);
            values
        },
        4096,
    );
}
