//! Source-bound installed expressions keep affine storage and ordinary offers.
use conduit_core as c;
use conduit_environment_make::NativeSoftwareSourceRecipe;
use conduit_environment_runtime::appliance::{
    assigned::{
        NativePreparationRefusal, PreparedResidentSoftware, ResidentExpressionMaterial,
        ResidentSelectorSchemas,
    },
    prepare_value_storage,
};
use conduit_kernel::{FixedValueStore, ValueStorage};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn descriptors(
    recipe: &NativeSoftwareSourceRecipe,
) -> (
    Vec<ResidentExpressionMaterial>,
    Vec<ResidentSelectorSchemas>,
) {
    let spec = recipe.spec();
    let mut expressions = Vec::new();
    let mut schemas = Vec::new();
    for t in recipe.resident_computations() {
        let expected = spec
            .computations
            .requirements
            .iter()
            .find(|r| r.node == t.node && r.key == t.key)
            .unwrap();
        if t.key == c::AssignedIdentity::from_text("program") {
            let canonical = t.canonical_bytes.unwrap();
            let p =
                conduit_plot::PortableExpressionProgram::from_canonical_bytes(canonical).unwrap();
            let m = conduit_plot::MakeExpressionMaterial::export(&p).unwrap();
            let bytes = Box::leak(m.bytes().to_vec().into_boxed_slice());
            expressions.push(ResidentExpressionMaterial {
                source: spec.source,
                node: t.node,
                key: t.key,
                text_digest: expected.text_digest,
                program_digest: c::assigned_plan_payload_digest(canonical),
                material_digest: c::assigned_plan_payload_digest(bytes),
                maximum_input_bytes: p.maximum_prepared_input_bytes().unwrap(),
                maximum_output_bytes: p.maximum_prepared_output_bytes().unwrap(),
                bytes,
            });
        } else {
            let s =
                c::StructuredSelector::from_canonical_bytes(t.canonical_bytes.unwrap()).unwrap();
            schemas.push(ResidentSelectorSchemas {
                node: t.node,
                key: t.key,
                text_digest: expected.text_digest,
                input_schema: Box::leak(
                    s.input_type().canonical_bytes().unwrap().into_boxed_slice(),
                ),
                output_schema: Box::leak(
                    s.output_type()
                        .canonical_bytes()
                        .unwrap()
                        .into_boxed_slice(),
                ),
            });
        }
    }
    (expressions, schemas)
}
#[test]
fn linked_make_prepares_the_exact_offers_with_lower_retention_and_peak() {
    let source = conduit_environment_runtime::construction::compile(960, 60000).unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let spec = recipe.spec();
    let texts: Vec<_> = recipe.resident_computations().collect();
    let (materials, schemas) = descriptors(&recipe);
    let values = prepare_value_storage().unwrap();
    let (owned, a) = allocation_probe::observe(|| {
        PreparedResidentSoftware::prepare_source_with_static_selectors_retained(
            &spec, &texts, &schemas, values,
        )
        .unwrap_or_else(|_| panic!("ordinary"))
    });
    let values = prepare_value_storage().unwrap();
    // SAFETY: fixture material is exported from this same ordinarily checked
    // Source, with independent exact configuration and material pins above.
    let (linked, b) = allocation_probe::observe(|| {
        unsafe {
            PreparedResidentSoftware::prepare_source_with_linked_make_retained(
                &spec, &texts, &schemas, &materials, values,
            )
        }
        .unwrap_or_else(|_| panic!("linked"))
    });
    let offers = recipe.resident_offers();
    assert_eq!(
        owned
            .source_offers(&spec, &offers)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        linked
            .source_offers(&spec, &offers)
            .unwrap()
            .iter()
            .collect::<Vec<_>>()
    );
    assert!(a.retained_bytes() > b.retained_bytes() + 6000);
    assert!(a.peak_bytes > b.peak_bytes);
    println!("resident ordinary_retained={} linked_retained={} ordinary_peak={} linked_peak={} image_bytes={}",a.retained_bytes(),b.retained_bytes(),a.peak_bytes,b.peak_bytes,materials.iter().map(|m|m.bytes.len()).sum::<usize>());
    let mut generated = String::new();
    recipe.write_source_rust(&mut generated).unwrap();
    assert!(generated.contains("static EXPRESSION_MATERIAL_BYTES:"));
    assert!(generated.contains("ResidentExpressionMaterial; 3]"));
    for m in materials {
        assert!(generated.contains(&format!("program_digest:{:?}", m.program_digest)));
    }
}
#[test]
fn every_descriptor_binding_refusal_preserves_the_actual_live_store() {
    let source = conduit_environment_runtime::construction::compile(3, 60000).unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let spec = recipe.spec();
    let texts: Vec<_> = recipe.resident_computations().collect();
    let (valid, schemas) = descriptors(&recipe);
    let mut cases = Vec::new();
    let mut m = valid.clone();
    m.pop();
    cases.push(m);
    let mut m = valid.clone();
    m.push(m[0]);
    cases.push(m);
    let mut m = valid.clone();
    m[1] = m[0];
    cases.push(m);
    for corrupt in 0..9 {
        let mut m = valid.clone();
        match corrupt {
            0 => m[0].source.expanded_plot_digest[0] ^= 1,
            1 => m[0].node = 15,
            2 => m[0].key = c::AssignedIdentity::from_text("selector"),
            3 => m[0].text_digest[0] ^= 1,
            4 => m[0].program_digest[0] ^= 1,
            5 => m[0].material_digest[0] ^= 1,
            6 => m[0].maximum_input_bytes += 1,
            7 => m[0].maximum_output_bytes += 1,
            _ => {
                let mut b = m[0].bytes.to_vec();
                b[0] ^= 1;
                m[0].bytes = Box::leak(b.into_boxed_slice());
                m[0].material_digest = c::assigned_plan_payload_digest(m[0].bytes);
            }
        }
        cases.push(m);
    }
    for bad in cases {
        let mut values = FixedValueStore::<2, 128>::new(128).unwrap();
        let reference = values.store(&[42; 64]).unwrap();
        let mut returned = match unsafe {
            PreparedResidentSoftware::prepare_source_with_linked_make_retained(
                &spec, &texts, &schemas, &bad, values,
            )
        } {
            Err((_, v)) => v,
            Ok(_) => panic!("bad descriptor accepted"),
        };
        assert_eq!(returned.get(reference).unwrap(), &[42; 64]);
        assert_eq!(returned.store(b"untouched slot").unwrap().generation, 1);
    }
    let mut values = FixedValueStore::<1, 64>::new(64).unwrap();
    let reference = values.store(&[42; 64]).unwrap();
    match unsafe {
        PreparedResidentSoftware::prepare_source_with_linked_make_retained(
            &spec, &texts, &schemas, &valid, values,
        )
    } {
        Err((NativePreparationRefusal::Storage(_), v)) => {
            assert_eq!(v.get(reference).unwrap(), &[42; 64])
        }
        _ => panic!("valid material must reach original storage admission"),
    }
}
