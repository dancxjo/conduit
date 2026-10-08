//! Development fixture export; no pretrained model, voice, or shared-IR claim.
use super::*;

#[test]
#[ignore = "explicit bounded synthetic guest preparation artifact export"]
fn export_synthetic_epoch_guest_preparation_materials() {
    use conduit_ai::fixed_numeric_binding::FixedTensorPortBinding;
    use sha2::{Digest, Sha256};
    let directory = std::path::PathBuf::from(
        std::env::var("CONDUIT_NUMERIC_GUEST_FIXTURE").expect("explicit fixture directory"),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let (plan, context) = prepared_epoch_plan(true).unwrap();
    super::plan_artifact::export(&directory, &plan, &context);
    let definition = super::declarations::exact_epoch_declarations();
    let checked =
        check_syntax_document(&parse_syntax_document(&definition), &StartupCatalog::new()).unwrap();
    std::fs::write(directory.join("native-definition.conduit"), &definition).unwrap();
    let native = context
        .native
        .iter()
        .map(|profile| {
            let name = &checked
                .native_types
                .iter()
                .find(|ty| &ty.value_type == profile.value_type())
                .unwrap()
                .name;
            serde_json::json!({"name": name, "kind": profile.kind_identity(true)})
        })
        .collect::<Vec<_>>();
    let weakening = context.weakening.iter().map(|profile| serde_json::json!({"input_type_canonical":profile.input_type().canonical_bytes().unwrap(),"kind":profile.kind_identity(true)})).collect::<Vec<_>>();
    let guards = context.guards.iter().map(|profile| serde_json::json!({"input_type_canonical":profile.value_type().canonical_bytes().unwrap(),"kind":profile.contract().unwrap().kind_id})).collect::<Vec<_>>();
    let pairs = context.pairs.iter().map(|profile| {
        let StructuredInfoTypeShape::Record {fields,..}=profile.value_type().shape() else {panic!("exact tuple")};
        assert_eq!(fields.len(),2);
        serde_json::json!({"left_type_canonical":fields[0].value_type().canonical_bytes().unwrap(),"right_type_canonical":fields[1].value_type().canonical_bytes().unwrap(),"kind":profile.identity()})
    }).collect::<Vec<_>>();
    let resources = super::synthetic_resources::synthetic_resources(&plan);
    assert_eq!(resources.len(), 26);
    let mut resource_rows = vec![];
    let mut resource_bytes = 0usize;
    for (index, (name, resource)) in resources.iter().enumerate() {
        let content_file = format!("resource-{index:02}.bin");
        std::fs::write(directory.join(&content_file), &resource.bytes).unwrap();
        let admitted = resource.adopt();
        assert_eq!(admitted.bytes(), resource.bytes.as_ref());
        let binding =
            FixedTensorPortBinding::prepare(&resource.value_type, &resource.tensor).unwrap();
        resource_bytes += resource.bytes.len();
        resource_rows.push(serde_json::json!({"name":name,"type_canonical":resource.value_type.canonical_bytes().unwrap(),"descriptor":resource.tensor.encode().unwrap(),"guest_access":"fresh synthetic Root read grant required","content_file":content_file,"content_bytes":resource.bytes.len(),"content_sha256":format!("{:x}",Sha256::digest(&resource.bytes)),"port_binding":binding.encoded()}));
    }
    let input = context
        .native
        .iter()
        .find(|profile| profile.kind_identity(true) == context.kinds["__EPOCH_INPUT_VALIDATOR__"])
        .unwrap()
        .value_type();
    let encoded = super::declarations::fixture_value(input)
        .canonical_bytes()
        .unwrap();
    std::fs::write(directory.join("input-value.bin"), &encoded).unwrap();
    let fixtures = plan.fragments[0]
        .placements
        .iter()
        .filter(|gear| gear.kind_id.as_str().starts_with("epoch-proof/"))
        .cloned()
        .collect::<Vec<_>>();
    let recipe = serde_json::json!({"schema":"conduit/synthetic-numeric-guest-preparation@1","entry_plot":"epoch-runtime-proof","u16_profile_source":"type FarganPeriod = U16 in 32..=255\n","scope":"synthetic numerical topology only; reference fixture Boot is not guest Boot; no model/voice admission", "native":native,"weakening":weakening,"guards":guards,"pairs":pairs,"reference_fixture_placements":fixtures,"resources":resource_rows,"inputs":[{"name":"value","file":"input-value.bin","bytes":encoded.len(),"sha256":format!("{:x}",Sha256::digest(&encoded))}],"resource_bytes":resource_bytes,"source_document_id":plan.source_document_id.as_str(),"reference_plan_id":plan.plan_id.as_str()});
    std::fs::write(
        directory.join("preparation-recipe.json"),
        serde_json::to_vec_pretty(&recipe).unwrap(),
    )
    .unwrap();
    eprintln!("synthetic guest fixture:26 resources/{resource_bytes} bytes,input{} bytes,6 native profiles; no guest execution",encoded.len());
}
