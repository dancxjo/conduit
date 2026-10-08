use super::*;
use alloc::vec::Vec;

#[test]
#[ignore = "explicit retained synthetic guest fixture, preparation only"]
fn same_checked_source_plans_for_current_boot_without_rewriting_reference() {
    let directory =
        std::path::PathBuf::from(std::env::var("CONDUIT_NUMERIC_GUEST_FIXTURE").unwrap());
    let source = std::fs::read(directory.join("checked-epoch-source.conduit")).unwrap();
    let image = std::fs::read(directory.join("sealed-epoch-plan.json")).unwrap();
    let definition = std::fs::read_to_string(directory.join("native-definition.conduit")).unwrap();
    let recipe_bytes = std::fs::read(directory.join("preparation-recipe.json")).unwrap();
    let data = Materials {
        source: &source,
        reference_image: &image,
        native_definition: &definition,
        recipe: &recipe_bytes,
    };
    let topology =
        PreparedTopology::prepare(data, "current/host".into(), "current/boot".into()).unwrap();
    let mut retained_files = Vec::new();
    for entry in std::fs::read_dir(&directory).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.ends_with(".bin") {
            retained_files.push((name, std::fs::read(entry.path()).unwrap()));
        }
    }
    let files: Vec<_> = retained_files
        .iter()
        .map(|(name, bytes)| resources::File { name, bytes })
        .collect();
    let ingress = resources::PreparedIngress::prepare(&topology, &files).unwrap();
    assert_eq!(ingress.resources.len(), 26);
    assert_eq!(ingress.values.len(), 27);
    assert_eq!(ingress.raw_resource_bytes, 2_989_684);
    assert_eq!(
        ingress.descriptor_inline_bytes,
        26 * core::mem::size_of::<conduit_data::TensorValue>()
    );
    assert!(
        ingress
            .values
            .values()
            .all(|bytes| bytes.len() <= storage::CELL_BYTES)
    );
    assert!(resources::PreparedIngress::prepare(&topology, &[]).is_err());
    let reference: Plan = serde_json::from_slice(&image).unwrap();
    assert_ne!(topology.plan().plan_id, reference.plan_id);
    assert_eq!(
        topology.plan().source_document_id,
        reference.source_document_id
    );
    assert_eq!(topology.plan().checked_plot_id, reference.checked_plot_id);
    assert_eq!(topology.plan().expanded_plot_id, reference.expanded_plot_id);
    assert_eq!(
        topology.plan().fragments[0].host_id.as_str(),
        "current/host"
    );
    assert_eq!(
        topology.plan().fragments[0].boot_id.as_str(),
        "current/boot"
    );
    assert_eq!(topology.plan().fragments[0].placements.len(), 770);
    assert_eq!(topology.plan().fragments[0].connections.len(), 1217);
    assert_eq!(topology.admitted_profile_counts(), [6, 3, 1, 2]);
    assert_eq!(topology.recipe_counts(), [26, 1, 28]);
    assert_eq!(
        topology.retained_material_extents(),
        [
            source.len(),
            image.len(),
            definition.len(),
            recipe_bytes.len()
        ]
    );
    let mut changed: serde_json::Value = serde_json::from_slice(&recipe_bytes).unwrap();
    changed["source_document_id"] = "foreign/source".into();
    let changed = serde_json::to_vec(&changed).unwrap();
    assert!(
        PreparedTopology::prepare(
            Materials {
                source: &source,
                reference_image: &image,
                native_definition: &definition,
                recipe: &changed
            },
            "current/host".into(),
            "current/boot".into()
        )
        .is_err()
    );
    eprintln!(
        "synthetic current-Boot Source planning:770nodes/1217cords; fullchecked/expanded correspondence; not guest execution"
    );
}

#[test]
fn recipe_refuses_capacity_and_escaping_file_names() {
    let bytes = alloc::vec![b' ';2*1024*1024+1];
    assert!(recipe::Recipe::decode(&bytes).is_err());
    let mut value = serde_json::json!({"schema":"conduit/synthetic-numeric-guest-preparation@1","entry_plot":"proof","u16_profile_source":"type Scalar = U16\n","scope":"synthetic","native":[],"weakening":[],"guards":[],"pairs":[],"reference_fixture_placements":[],"resources":[],"inputs":[],"resource_bytes":0,"source_document_id":"source","reference_plan_id":"plan"});
    assert!(recipe::Recipe::decode(&serde_json::to_vec(&value).unwrap()).is_ok());
    value["inputs"] = serde_json::json!([{"name":"input","file":"../secret.bin","bytes":1,"sha256":"00".repeat(32)}]);
    assert!(recipe::Recipe::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    value["inputs"] = serde_json::json!([{"name":"input","file":"input.bin","bytes":16385,"sha256":"00".repeat(32)}]);
    assert!(recipe::Recipe::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    value["inputs"] = serde_json::json!([]);
    value["resources"] = serde_json::json!([]);
    value["extra_authority"] = true.into();
    assert!(recipe::Recipe::decode(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn native_definition_capacity_is_refused_before_reference_decode() {
    let definition =
        " ".repeat(conduit_ai::native_profile::MAXIMUM_NATIVE_PROFILE_SOURCE_BYTES + 1);
    let error = PreparedTopology::prepare(
        Materials {
            source: &[],
            reference_image: &[],
            native_definition: &definition,
            recipe: &[],
        },
        "host".into(),
        "boot".into(),
    )
    .err()
    .unwrap();
    assert_eq!(error, "numeric native definition selected capacity");
}
#[test]
#[cfg(feature = "numeric-topology-catalog-cache")]
fn immutable_catalog_clones_cannot_mutate_checked_cache() {
    use conduit_ai::fixed_numeric_catalog::*;
    let started = std::time::Instant::now();
    let mut types = fixed_numeric_types().unwrap();
    let name = types[0].name.clone();
    let expected = fixed_numeric_type(&name).unwrap();
    let cold = started.elapsed();
    types[0].name = "foreign/cache".into();
    let mut contracts = fixed_numeric_contracts().unwrap();
    let first = contracts[0].kind_id.clone();
    contracts[0].kind_id = kind_id("foreign/cache");
    let started = std::time::Instant::now();
    assert_eq!(fixed_numeric_type(&name).unwrap(), expected);
    assert!(fixed_numeric_type("foreign/cache").is_err());
    let warm = started.elapsed();
    assert_eq!(fixed_numeric_contracts().unwrap()[0].kind_id, first);
    std::eprintln!(
        "immutable checked metadata selected-Type cold={cold:?} warm={warm:?}; host diagnostic only"
    );
}
