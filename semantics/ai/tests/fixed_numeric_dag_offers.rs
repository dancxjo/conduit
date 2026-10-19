#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{fixed_numeric_catalog::*, operation_owners::fixed_numeric::*};
use conduit_core::*;
use conduit_plot::*;
use std::collections::BTreeMap;
#[allow(dead_code)]
#[path = "fargan_signal_graph/shared.rs"]
mod shared;
fn fixture() -> (
    ExpandedAuthoringPlot,
    HostAdvertisement,
    BTreeMap<GearId, NumericDagPlacement>,
) {
    fixture_with_tail(0)
}
fn fixture_with_tail(
    tail: usize,
) -> (
    ExpandedAuthoringPlot,
    HostAdvertisement,
    BTreeMap<GearId, NumericDagPlacement>,
) {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs_capacity64(&mut startup, &mut profiles).unwrap();
    let mut capabilities = Vec::new();
    for (name, ty, source) in [
        ("test/input", "NumericF32Vector192", true),
        ("test/weights", "NumericF32MatrixRef192x4", true),
        ("test/output", "NumericF32Vector4", false),
    ] {
        let kind = shared::fixture_kind(name, &fixed_numeric_type(ty).unwrap(), source);
        startup
            .insert(KindSignature {
                kind: name.into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profiles.insert_kind(kind.clone()).unwrap();
        capabilities.push(shared::offer(kind));
    }
    for kind in ["numeric/tanh192", "numeric/linear192x4", "numeric/tanh4"] {
        capabilities.push(fixed_numeric_offer_capacity64(kind).unwrap());
    }
    let source = "plot test {\n input: test/input\n weights: test/weights\n a: numeric/tanh192\n b: numeric/linear192x4\n c: numeric/tanh4\n output: test/output\n input.value >> a.value\n a.result >> b.value\n weights.value >> b.weights\n b.result >> c.value\n c.result >> output.value\n}\n";
    let source = if tail == 0 {
        source.to_owned()
    } else {
        let mut extension = String::new();
        let mut previous = "c".to_owned();
        for n in 0..tail {
            let name = format!("tail{n}");
            extension.push_str(&format!(
                " {name}: numeric/tanh4\n {previous}.result >> {name}.value\n"
            ));
            previous = name;
        }
        extension.push_str(&format!(" {previous}.result >> output.value\n"));
        source.replace(" c.result >> output.value\n", &extension)
    };
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "test", &profiles).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "test".into(),
        boot_id: "test-boot".into(),
        offer_generation: OfferGeneration(7),
        profile: "numeric-test@1".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    };
    let placements = conduit_planner::default_expanded_placements(
        &expanded.expanded,
        std::slice::from_ref(&host),
    )
    .unwrap();
    let selected = placements
        .by_gear
        .into_iter()
        .map(|(id, p)| {
            (
                id,
                NumericDagPlacement {
                    host_id: p.host_id,
                    capability_id: p.capability_id,
                },
            )
        })
        .collect();
    (expanded, host, selected)
}
#[test]
fn distinct_widths_form_one_exact_region_with_external_canonical_weights() {
    let (plot, host, selected) = fixture();
    for transport in [NumericDagTransport::Canonical, NumericDagTransport::Compact] {
        let (offers, kinds) = numeric_dag_offers(&plot, &host, &selected, transport).unwrap();
        assert_eq!(offers.len(), 1);
        let offer = &offers[0];
        assert_eq!(offer.gear_ids.len(), 3);
        assert_eq!(offer.internal_cords.len(), 2);
        assert_eq!(kinds.len(), 3);
        assert_eq!(offer.boot_id, host.boot_id);
        assert_eq!(offer.offer_generation, host.offer_generation);
        assert_eq!(offer.artifact_id.as_str(), MATRIX_DAG_ARTIFACT);
        assert!(offer.gear_ids.iter().all(|id| plot
            .expanded
            .gears
            .iter()
            .find(|g| &g.gear_id == id)
            .unwrap()
            .kind_id
            .as_str()
            .starts_with("numeric/")));
    }
}
#[test]
fn missing_or_ambiguous_selected_identity_is_rejected() {
    let (plot, host, mut selected) = fixture();
    selected.pop_first();
    assert!(numeric_dag_offers(&plot, &host, &selected, NumericDagTransport::Compact).is_err());
    let (plot, mut host, selected) = fixture();
    host.capabilities.push(host.capabilities[0].clone());
    assert!(numeric_dag_offers(&plot, &host, &selected, NumericDagTransport::Compact).is_err());
}
#[test]
fn forged_owned_offer_and_remote_weights_never_admit_matrix() {
    for mutation in 0..3 {
        let (plot, mut host, mut selected) = fixture();
        if mutation < 2 {
            let linear = host
                .capabilities
                .iter_mut()
                .find(|o| o.kind_id.as_str() == "numeric/linear192x4")
                .unwrap();
            if mutation == 0 {
                linear.implementation.artifact_id = "forged".into();
            } else {
                linear.kind_contract_revision = "old-contract".into();
            }
        } else {
            let weights = plot
                .expanded
                .gears
                .iter()
                .find(|g| g.kind_id.as_str() == "test/weights")
                .unwrap();
            selected.get_mut(&weights.gear_id).unwrap().host_id = "remote".into();
        }
        let (offers, _) =
            numeric_dag_offers(&plot, &host, &selected, NumericDagTransport::Compact).unwrap();
        assert!(offers.is_empty());
    }
}

#[test]
fn long_components_are_partitioned_into_disjoint_bounded_exact_groups() {
    let (plot, host, selected) = fixture_with_tail(60);
    let (offers, _) =
        numeric_dag_offers(&plot, &host, &selected, NumericDagTransport::Compact).unwrap();
    assert!(offers.len() >= 2);
    let mut seen = std::collections::BTreeSet::new();
    for offer in offers {
        assert!((2..=32).contains(&offer.gear_ids.len()));
        for gear in &offer.gear_ids {
            assert!(seen.insert(gear.clone()));
        }
        assert_eq!(offer.internal_cords.len(), offer.gear_ids.len() - 1);
        for (a, b) in &offer.internal_cords {
            assert!(offer.gear_ids.contains(a) && offer.gear_ids.contains(b));
            assert!(plot
                .expanded
                .connections
                .iter()
                .filter(|c| &c.source_gear_id == a)
                .all(|c| offer.gear_ids.contains(&c.sink_gear_id)));
        }
    }
    assert_eq!(seen.len(), 63);
}
#[test]
fn exposed_matrix_output_remains_a_canonical_boundary() {
    let (mut plot, host, selected) = fixture();
    let matrix = plot
        .expanded
        .gears
        .iter()
        .find(|g| g.kind_id.as_str() == "numeric/linear192x4")
        .unwrap()
        .gear_id
        .clone();
    plot.output_bindings.push(AuthoringFrontBinding {
        front_port_id: port_id("observed"),
        gear_id: matrix.clone(),
        gear_port_id: port_id("result"),
        track: ConnectionTrack::Payload,
    });
    let (offers, _) =
        numeric_dag_offers(&plot, &host, &selected, NumericDagTransport::Compact).unwrap();
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0].gear_ids.len(), 2);
    assert!(offers[0]
        .internal_cords
        .iter()
        .all(|(source, _)| source != &matrix));
}

#[test]
fn only_exact_reviewed_slice_profiles_admit_distinct_width_transport() {
    for (i, o) in [(480, 160), (384, 128)] {
        let name = format!("numeric/slice{i}x{o}");
        for offer in [
            fixed_numeric_offer(&name).unwrap(),
            fixed_numeric_offer_capacity64(&name).unwrap(),
        ] {
            let layout = compact_slice_offer_layout(&offer).unwrap();
            assert_eq!(layout.numeric_input.1.lanes, i);
            assert_eq!(layout.output.1.lanes, o);
            assert_eq!(layout.canonical_inputs, [conduit_kernel::PortId(1)]);
            for mutation in 0..4 {
                let mut changed = offer.clone();
                match mutation {
                    0 => changed.kind_contract_revision = "old".into(),
                    1 => changed.implementation.artifact_id = "forged".into(),
                    2 => changed.inputs[1].port_id = port_id("forged"),
                    _ => changed.outputs[0].value_kind = kind_id("forged"),
                }
                assert!(compact_slice_offer_layout(&changed).is_err());
            }
        }
    }
    assert!(
        compact_slice_offer_layout(&fixed_numeric_offer("numeric/linear192x4").unwrap()).is_err()
    );
}
