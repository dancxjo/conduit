//! Actual combined authored protocol construction, not admission or hardware proof.
use super::*;

use crate as source_build;
#[path = "../tests/fixtures/environmental_exchange_source.rs"]
mod environmental_exchange_source;

#[test]
fn actual_environment_protocols_share_one_correlated_controller_and_clock() {
    let fixture = environmental_exchange_source::selected_source();
    let ids = &fixture.kind_ids;
    let workset = fixture.compile().unwrap();
    let direct = crate::workset::compile_specialized(
        fixture.source.as_bytes(),
        &fixture.source_sha256,
        &["shared-environment".into()],
        None,
        &environmental_exchange_source::specializations(),
    )
    .unwrap();
    assert_eq!(workset.entries[0].rust, direct.entries[0].rust);
    assert_eq!(workset.sha256, direct.sha256);
    let residents = workset
        .prepare_selected_residents_with_exact_work(
            crate::resident_expression_bounds(65_000),
            conduit_embedded_build::ResidentExecutionStorage::SharedWorkspaceSchema,
            65_000,
        )
        .unwrap();
    assert!(
        residents.len()
            > residents
                .iter()
                .map(|image| image.kind_id())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
        "fixture must exercise repeated checked programs"
    );
    let work: Vec<_> = residents
        .iter()
        .map(|image| u32::from(image.computation_work()))
        .collect();
    let total: u32 = work.iter().copied().sum();
    assert!(
        work.iter().max() > work.iter().min(),
        "unequal owners must keep unequal requirements"
    );
    assert!(
        work.iter().max().copied().unwrap() > 65_000 / residents.len() as u32,
        "fixture must exceed the old uniform per-owner share"
    );
    assert!(residents.iter().all(
        |image| image.offer_description().resource_requirements[0].units
            == u32::from(image.computation_work())
    ));
    assert!(workset
        .prepare_selected_residents_with_exact_work(
            crate::resident_expression_bounds(65_000),
            conduit_embedded_build::ResidentExecutionStorage::SharedWorkspaceSchema,
            total
        )
        .is_ok());
    assert!(workset
        .prepare_selected_residents_with_exact_work(
            crate::resident_expression_bounds(65_000),
            conduit_embedded_build::ResidentExecutionStorage::SharedWorkspaceSchema,
            total - 1
        )
        .err()
        .unwrap()
        .contains("original pool"));
    eprintln!(
        "resident owners={} private_total={total} largest={:?}",
        residents.len(),
        work.iter().max()
    );
    let capabilities = residents
        .iter()
        .map(|image| &image.offer_description().capability_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        capabilities.len(),
        residents.len(),
        "each repeated Gear has a distinct initialized owner description"
    );
    let entry = &workset.entries[0];
    let gears = &entry.plot.expanded.gears;
    assert_eq!(
        gears
            .iter()
            .filter(|g| g.kind_id.as_str() == conduit_i2c_transaction::contract::I2C_KIND)
            .count(),
        1
    );
    assert_eq!(gears.iter().filter(|g| g.kind_id.as_str() == conduit_monotonic_wait::clock_at::contract::CLOCK_KIND).count(), 1);
    let distinct = ids.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(distinct.len(), 9);
    for id in ids {
        assert!(
            gears.iter().any(|g| g.kind_id == *id),
            "missing {}",
            id.as_str()
        );
    }
    // Two independent merge owners arbitrate three request lanes; neither is
    // another native bus nor an alias of initialized mutable Back storage.
    assert_eq!(gears.iter().filter(|g| g.kind_id == ids[4]).count(), 2);
    let material = crate::software::compile_with_types(
        &entry.plot.expanded,
        Some(&workset.checked_source),
        32 * 1024,
    )
    .unwrap();
    eprintln!(
        "combined protocol geometry: gears={}, cords={}, software_nodes={}, software_offer_bytes={}, generated_rust_bytes={}; not a firmware memory measurement",
        gears.len(), entry.cords, material.nodes.len(),
        material.nodes.iter().filter_map(|node|node.offer_image.as_ref()).map(Vec::len).sum::<usize>(),
        entry.rust.len(),
    );
    let offers = material
        .nodes
        .iter()
        .filter_map(|node| node.offer.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(
        offers
            .iter()
            .map(|offer| &offer.capability_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        offers.len()
    );
    assert!(offers
        .iter()
        .all(|offer| offer.limits.max_active_instances == 1));
    let merges = offers
        .iter()
        .filter(|offer| offer.kind_id == ids[4])
        .collect::<Vec<_>>();
    assert_eq!(merges.len(), 2);
    assert_eq!(merges[0].implementation, merges[1].implementation);
    assert_ne!(merges[0].capability_id, merges[1].capability_id);
    for node in &material.nodes {
        if let Some(offer) = &node.offer {
            assert_eq!(offer.kind_id, node.kind);
            assert!(node.offer_image.is_some());
            if let Some((constructed, _)) = crate::composite::selected(
                &gears[usize::from(node.node)],
                Some(&workset.checked_source),
            )
            .unwrap()
            {
                assert_eq!(
                    offer.limits.max_queue_items,
                    constructed.limits.max_queue_items
                );
                assert_eq!(
                    offer.limits.max_queue_bytes,
                    constructed.limits.max_queue_bytes
                );
            }
        }
    }
    assert!(entry.rust.contains("SeededUntilStatic"));
    assert!(entry.rust.contains("MergeStatic"));
    assert!(entry.rust.contains("ZipStatic"));
    routing_transcript(&workset.checked_source);
    assert!(entry
        .plot
        .output_bindings
        .iter()
        .any(|binding| binding.front_port_id.as_str() == "unrouted"));
}

fn routing_transcript(doc: &p::CheckedSyntaxDocument) {
    let expanded = p::expand_canonical_plot_for_authoring(
        doc,
        "environmental-route-result",
        &p::ProfileCatalog::new(),
    )
    .unwrap();
    let c::ConfigurationValue::Text(program) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("routing expression")
    };
    let mut evaluator = p::PreparedPortableExpressionEvaluator::new(
        &p::PortableExpressionProgram::from_canonical_hex(program).unwrap(),
    )
    .unwrap();
    let schema = |name: &str| {
        doc.native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
            .clone()
    };
    let correlation_type = schema("I2cCorrelation");
    let result_type = schema("I2cCorrelatedResult");
    let record = |ty: &c::StructuredInfoType, fields: Vec<(&str, c::StructuredInfoValue)>| {
        c::StructuredInfoValue::record(
            ty.clone(),
            fields
                .into_iter()
                .map(|(name, value)| c::StructuredFieldValue::new(name, value).unwrap())
                .collect(),
        )
        .unwrap()
    };
    let scalar = |value: u64| {
        c::StructuredInfoValue::leaf(
            c::StructuredInfoType::leaf(c::kind_id("value/u64")).unwrap(),
            value.to_le_bytes().to_vec(),
        )
        .unwrap()
    };
    let contract = conduit_i2c_transaction::contract::I2cContract::prepare().unwrap();
    let mut codec =
        conduit_i2c_transaction::result::PreparedI2cResultEncoder::new(&contract).unwrap();
    let bus_result = codec
        .disposition(conduit_i2c_transaction::I2cDisposition::NotAcknowledged)
        .unwrap()
        .to_vec();
    for (client, sequence, expected_tag) in [
        (1, 0, "sensor"),
        (2, 0, "initialization"),
        (3, 0, "write"),
        (4, 0, "unrouted"),
        (1, 1, "unrouted"),
    ] {
        let correlation = record(
            &correlation_type,
            vec![("client", scalar(client)), ("sequence", scalar(sequence))],
        );
        let input = record(
            &result_type,
            vec![
                ("correlation", correlation),
                (
                    "result",
                    c::StructuredInfoValue::from_canonical_bytes(&bus_result).unwrap(),
                ),
            ],
        )
        .canonical_bytes()
        .unwrap();
        let output =
            c::StructuredInfoValue::from_canonical_bytes(evaluator.evaluate(&input).unwrap())
                .unwrap();
        let c::StructuredInfoValueShape::Variant { tag, payload } = output.shape() else {
            panic!("routing result")
        };
        assert_eq!(tag, expected_tag);
        assert_eq!(
            payload.canonical_bytes().unwrap(),
            if expected_tag == "unrouted" {
                &input
            } else {
                &bus_result
            }
            .as_slice()
        );
    }
}
