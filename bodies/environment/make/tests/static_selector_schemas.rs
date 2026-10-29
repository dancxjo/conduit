//! Static linked schema storage preserves ordinary Source/configuration admission.
use conduit_environment_make::NativeSoftwareSourceRecipe;
use conduit_environment_runtime::appliance::{
    assigned::{NativePreparationRefusal, PreparedResidentSoftware, ResidentSelectorSchemas},
    prepare_value_storage,
};
use conduit_kernel::{FixedValueStore, ValueStorage};
use std::collections::BTreeMap;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn schemas(recipe: &NativeSoftwareSourceRecipe) -> Vec<ResidentSelectorSchemas> {
    let mut types = BTreeMap::<Vec<u8>, &'static [u8]>::new();
    recipe
        .resident_computations()
        .filter(|text| text.key == conduit_core::AssignedIdentity::from_text("selector"))
        .map(|text| {
            let selector = conduit_core::StructuredSelector::from_canonical_hex(text.text).unwrap();
            let mut intern = |value: &conduit_core::StructuredInfoType| {
                let bytes = value.canonical_bytes().unwrap();
                *types
                    .entry(bytes.clone())
                    .or_insert_with(|| Box::leak(bytes.into_boxed_slice()))
            };
            ResidentSelectorSchemas {
                node: text.node,
                key: text.key,
                text_digest: recipe
                    .spec()
                    .computations
                    .requirements
                    .iter()
                    .find(|c| c.node == text.node && c.key == text.key)
                    .unwrap()
                    .text_digest,
                input_schema: intern(selector.input_type()),
                output_schema: intern(selector.output_type()),
            }
        })
        .collect()
}
#[test]
fn shared_static_types_prepare_same_source_offers_with_less_retained_storage() {
    let source = conduit_environment_runtime::construction::compile(960, 60000).unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let spec = recipe.spec();
    let texts = recipe.resident_computations().collect::<Vec<_>>();
    let schemas = schemas(&recipe);
    let owned_values = prepare_value_storage().unwrap();
    let static_values = prepare_value_storage().unwrap();
    let (owned, a) = allocation_probe::observe(|| {
        PreparedResidentSoftware::prepare_source_retained(&spec, &texts, owned_values)
            .unwrap_or_else(|_| panic!("owned preparation"))
    });
    let (linked, b) = allocation_probe::observe(|| {
        PreparedResidentSoftware::prepare_source_with_static_selectors_retained(
            &spec,
            &texts,
            &schemas,
            static_values,
        )
        .unwrap_or_else(|_| panic!("static preparation"))
    });
    let offers = recipe.resident_offers();
    let old = owned
        .source_offers(&spec, &offers)
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    let new = linked
        .source_offers(&spec, &offers)
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(old, new);
    let type_bytes: usize = schemas
        .iter()
        .map(|s| s.input_schema.len() + s.output_schema.len())
        .sum();
    assert!(
        a.retained_bytes() >= b.retained_bytes() + type_bytes,
        "owned={a:?} static={b:?} schema={type_bytes}"
    );
    println!(
        "selector_schema_storage owned={} static={} canonical_bytes={} saved={}",
        a.retained_bytes(),
        b.retained_bytes(),
        type_bytes,
        a.retained_bytes() - b.retained_bytes()
    );
}
#[test]
fn static_descriptors_refuse_missing_duplicate_extra_digest_node_key_and_type_mismatch() {
    let source = conduit_environment_runtime::construction::compile(3, 60000).unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let spec = recipe.spec();
    let texts = recipe.resident_computations().collect::<Vec<_>>();
    let valid = schemas(&recipe);
    let mut cases = Vec::new();
    let mut v = valid.clone();
    v.pop();
    cases.push(v);
    let mut v = valid.clone();
    v.push(v[0]);
    cases.push(v);
    let mut v = valid.clone();
    v[1] = v[0];
    cases.push(v);
    let mut v = valid.clone();
    v[0].text_digest[0] ^= 1;
    cases.push(v);
    let mut v = valid.clone();
    v[0].node = u16::MAX;
    cases.push(v);
    let mut v = valid.clone();
    v[0].key = conduit_core::AssignedIdentity::from_text("program");
    cases.push(v);
    let mut v = valid.clone();
    v[0].input_schema = v[0].output_schema;
    cases.push(v);
    let mut v = valid.clone();
    v[0].output_schema = v[0].input_schema;
    cases.push(v);
    for bad in cases {
        let mut values = FixedValueStore::<1, 64>::new(64).unwrap();
        let reference = values.store(&[42; 64]).unwrap();
        match PreparedResidentSoftware::prepare_source_with_static_selectors_retained(
            &spec, &texts, &bad, values,
        ) {
            Err((_, values)) => assert_eq!(values.get(reference).unwrap(), &[42; 64]),
            Ok(_) => panic!("bad schema accepted"),
        }
    }
    let mut values = FixedValueStore::<1, 64>::new(64).unwrap();
    let reference = values.store(&[42; 64]).unwrap();
    match PreparedResidentSoftware::prepare_source_with_static_selectors_retained(
        &spec, &texts, &valid, values,
    ) {
        Err((NativePreparationRefusal::Storage(_), values)) => {
            assert_eq!(values.get(reference).unwrap(), &[42; 64])
        }
        _ => panic!("valid schemas must reach unchanged storage admission"),
    }
}
#[test]
fn checked_make_emits_one_deduplicated_type_blob_and_exact_source_bindings() {
    let source = conduit_environment_runtime::construction::compile(3, 60000).unwrap();
    let recipe = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let expected = schemas(&recipe);
    let mut generated = String::new();
    recipe.write_source_rust(&mut generated).unwrap();
    let start = generated
        .find("static SELECTOR_SCHEMA_BYTES: &[u8] = &")
        .unwrap()
        + "static SELECTOR_SCHEMA_BYTES: &[u8] = &".len();
    let end = start + generated[start..].find(';').unwrap();
    let blob: Vec<u8> = serde_json::from_str(&generated[start..end]).unwrap();
    let unique = expected
        .iter()
        .flat_map(|s| [s.input_schema, s.output_schema])
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(blob.len(), unique.iter().map(|s| s.len()).sum::<usize>());
    for bytes in unique {
        assert!(blob.windows(bytes.len()).any(|window| window == bytes));
    }
    for schema in expected {
        assert!(generated.contains(&format!(
            "node: {}, key: c::AssignedIdentity({:?}), text_digest: {:?}",
            schema.node, schema.key.0, schema.text_digest
        )));
    }
    assert!(generated.contains("pub fn selector_schemas()"));
}
