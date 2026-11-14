//! CPU fabrication evidence, without environmental Source, hardware or admission.
use conduit_core as c;
use conduit_embedded_build as b;
use conduit_plot as p;

fn document() -> p::CheckedSyntaxDocument {
    let source = "plot increment (\n >> reading: U8\n result: U8 >>\n) = (reading + 1)\n\
                  plot decrement (\n >> reading: U8\n result: U8 >>\n) = (reading - 1)\n";
    p::check_syntax_document(&p::parse_syntax_document(source), &p::StartupCatalog::new()).unwrap()
}
fn bounds() -> b::CheckedWorksetBounds {
    b::CheckedWorksetBounds {
        maximum_entries: 2,
        maximum_nodes_per_entry: 1,
        maximum_cords_per_entry: 0,
    }
}
fn software_bounds() -> b::SelectedSoftwareBounds {
    b::SelectedSoftwareBounds {
        maximum_nodes: 1,
        maximum_offers: 1,
        maximum_image_bytes: 8192,
        maximum_total_image_bytes: 8192,
    }
}
fn offer(gear: &p::CheckedGear) -> c::CapabilityOffer {
    c::capability_offer_from_parts! {
        semantic_contract:gear.semantic_contract.clone(),startup_parameters:gear.startup_parameters.clone(),shorthand:gear.shorthand.clone(),
        capability_id:"fixture/increment".into(),kind_id:gear.kind_id.clone(),kind_contract_revision:gear.kind_contract_revision.clone(),
        implementation:c::ImplementationOffer {execution_profile_id:"fixture/pure".into(),implementation_id:"fixture/selected-implementation".into(),artifact_id:"fixture/selected-artifact".into()},
        inputs:gear.inputs.clone(),outputs:gear.outputs.clone(),host_calls:vec![],resource_requirements:vec![],authority_requirements:vec![],
        limits:c::CapabilityLimits {max_active_instances:1,max_queue_items:1,max_queue_bytes:1024},
    }
}

#[test]
fn explicit_entries_preserve_order_and_ordinary_expansion_identity() {
    let doc = document();
    let catalog = p::ProfileCatalog::new();
    let plots =
        b::prepare_checked_workset(&doc, &catalog, &["decrement", "increment"], bounds()).unwrap();
    assert_eq!(plots.len(), 2);
    for (plot, name) in plots.iter().zip(["decrement", "increment"]) {
        assert_eq!(
            *plot,
            p::expand_canonical_plot_for_authoring(&doc, name, &catalog).unwrap()
        );
        assert_eq!(plot.expanded.source_document_id, doc.source_document_id);
    }
}

#[test]
fn only_the_explicit_profile_catalog_supplies_native_meaning() {
    let kind = conduit_semantic_catalog::text_upper_contract()
        .into_semantic_contract("fixture/text-upper@1");
    let mut startup = p::StartupCatalog::new();
    startup
        .insert(p::KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
        .insert_fore(kind.kind_id.as_str(), kind.checked_front())
        .unwrap();
    let source = format!("plot uppercase (\n >> input: Text\n output: Text >>\n) {{\n upper: {}\n input >> upper >> output\n}}.\n", kind.kind_id.as_str());
    let doc = p::check_syntax_document(&p::parse_syntax_document(&source), &startup).unwrap();
    assert!(
        b::prepare_checked_workset(&doc, &p::ProfileCatalog::new(), &["uppercase"], bounds())
            .is_err()
    );
    let mut profiles = p::ProfileCatalog::new();
    profiles.insert_kind(kind).unwrap();
    let selected = b::prepare_checked_workset(&doc, &profiles, &["uppercase"], bounds()).unwrap();
    assert_eq!(selected[0].expanded.gears.len(), 1);
}

#[test]
fn absent_duplicate_and_oversized_selection_never_produces_a_workset() {
    let doc = document();
    let catalog = p::ProfileCatalog::new();
    for entries in [
        &[][..],
        &["missing"][..],
        &["increment", "increment"][..],
        &["increment", "decrement", "third"][..],
    ] {
        assert!(b::prepare_checked_workset(&doc, &catalog, entries, bounds()).is_err());
    }
    let mut small = bounds();
    small.maximum_nodes_per_entry = 0;
    assert!(b::prepare_checked_workset(&doc, &catalog, &["increment"], small).is_err());
}

#[test]
fn software_metadata_keeps_the_explicit_offer_and_configuration_losslessly() {
    let doc = document();
    let plots =
        b::prepare_checked_workset(&doc, &p::ProfileCatalog::new(), &["increment"], bounds())
            .unwrap();
    let source = &plots[0].expanded;
    let offer = offer(&source.gears[0]);
    let selected = [b::SelectedSoftwareOffer {
        node: 0,
        offer: &offer,
        configuration: &source.gears[0].configuration,
    }];
    let result = b::prepare_selected_software(source, &selected, software_bounds()).unwrap();
    assert!(core::ptr::eq(result[0].offer, &offer));
    assert_eq!(result[0].configuration, source.gears[0].configuration);
    let image = conduit_wire::DeflatedDocument::parse_image(&result[0].image).unwrap();
    let mut storage = vec![0; image.expanded_bytes()];
    let bytes = image
        .decode_into(
            &mut storage,
            conduit_wire::DeflatedDocumentLimits {
                maximum_expanded_bytes: 65536,
                maximum_compressed_bytes: 8192,
            },
        )
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<c::CapabilityOffer>(bytes).unwrap(),
        offer
    );
    let size = result[0].image.len();
    for per_image in [true, false] {
        let mut small = software_bounds();
        if per_image {
            small.maximum_image_bytes = size - 1;
        } else {
            small.maximum_total_image_bytes = size - 1;
        }
        assert!(b::prepare_selected_software(source, &selected, small).is_err());
    }
}

#[test]
fn stale_configuration_wrong_contract_and_duplicate_nodes_are_refused() {
    let doc = document();
    let plots = b::prepare_checked_workset(
        &doc,
        &p::ProfileCatalog::new(),
        &["increment", "decrement"],
        bounds(),
    )
    .unwrap();
    let source = &plots[0].expanded;
    let correct = offer(&source.gears[0]);
    let mut wrong = correct.clone();
    wrong.kind_id = "fixture/wrong-kind".into();
    for (offer, configuration) in [
        (&correct, &plots[1].expanded.gears[0].configuration),
        (&wrong, &source.gears[0].configuration),
    ] {
        assert!(b::prepare_selected_software(
            source,
            &[b::SelectedSoftwareOffer {
                node: 0,
                offer,
                configuration
            }],
            software_bounds()
        )
        .is_err());
    }
    for nodes in [[0, 0], [0, 1]] {
        let selected = nodes.map(|node| b::SelectedSoftwareOffer {
            node,
            offer: &correct,
            configuration: &source.gears[0].configuration,
        });
        let mut limits = software_bounds();
        limits.maximum_offers = 2;
        assert!(b::prepare_selected_software(source, &selected, limits).is_err());
    }
}
