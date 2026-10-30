use super::*;
use crate::{
    identity::BootIdentities,
    keyboard_offer::{KeyboardMechanism, KeyboardRealization},
    offer::{CpuFeatures, HostOffer},
};
use conduit_body::{Body, BodyWorkset, Wake};

pub(crate) fn fixture() -> (BootIdentities, HostOffer<'static>) {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        1_048_576,
    )
    .with_keyboard(
        KeyboardRealization {
            mechanism: KeyboardMechanism::UsbHid,
            controller_id: [3; 32],
            device_id: [4; 32],
            interface_id: [5; 32],
            endpoint_id: [6; 32],
            report_buffers: 2,
            transition_slots: 8,
            operation_slots: 2,
        },
        "build",
    )
    .unwrap();
    (identities, offer)
}

pub(crate) fn wake(plots: &[NativePlot]) -> Wake {
    let workset =
        BodyWorkset::from_plots(plots.iter().map(|plot| resident(*plot).unwrap())).unwrap();
    Body::born_with_plots(workset, 1, "sign/born".into())
        .unwrap()
        .wake(2, "sign/woke".into())
        .unwrap()
        .1
}

#[test]
fn native_inventory_preserves_existing_plots_and_adds_the_canonical_tour_identity() {
    let profile = profile();
    assert_eq!(profile.id, "conduitos/native-installed-plots@1");
    assert_eq!(profile.capacity, NATIVE_PLOT_CAPACITY);
    assert_eq!(profile.installed(), &inventory());
    let old = crate::keyboard_text_plan::checked_plot_identity().unwrap();
    let canvas = resident(NativePlot::KeyboardCanvas).unwrap();
    assert_eq!(canvas.source_document_id, old.source_document_id);
    assert_eq!(canvas.checked_plot_id, old.checked_plot_id);
    assert_ne!(canvas, resident(NativePlot::MemoryLantern).unwrap());
    assert_eq!(
        NativePlot::MemoryLantern.source(),
        include_str!("../../../../plots/memory-lantern/main.conduit")
    );
    assert_eq!(
        NativePlot::Tour.source(),
        include_str!("../../../../plots/tour/main.conduit")
    );
    assert_ne!(canvas, resident(NativePlot::Tour).unwrap());
    assert_eq!(
        catalog::resolve(&canvas).unwrap(),
        NativePlot::KeyboardCanvas
    );
    let mut substituted = canvas;
    substituted.checked_plot_id = "checked/substituted".into();
    assert_eq!(
        catalog::resolve(&substituted),
        Err(WorksetRefusal::UnknownPlot)
    );
}

#[test]
fn profile_capacity_is_the_finite_native_workset_bound() {
    assert_eq!(NATIVE_PLOT_CAPACITY, inventory().len());
    let one = BodyWorkset::from_plots(inventory()[..1].iter().map(|plot| resident(*plot).unwrap()))
        .unwrap();
    let maximum =
        BodyWorkset::from_plots(inventory().iter().map(|plot| resident(*plot).unwrap())).unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(maximum.len(), profile().capacity);
    assert!(BodyWorkset::default().is_empty());
}

#[test]
fn canonical_memory_plans_as_an_exact_native_body_partition_before_play() {
    let (ids, offer) = fixture();
    let wake = wake(&[NativePlot::MemoryLantern]);
    let prepared = prepare(&wake, &ids, &offer, "build").unwrap();
    prepared.plan.validate_for(&wake).unwrap();
    assert_eq!(prepared.plan.plots.len(), 1);
    assert_eq!(prepared.lowered.nodes, 2);
    assert_eq!(prepared.lowered.cords, 2);
    assert!(
        prepared.plan.plots[0].plan.fragments[0]
            .placements
            .iter()
            .any(|gear| gear.implementation_id.as_str() == document_state::IMPLEMENTATION)
    );
    assert_eq!(
        prepare(&wake, &ids, &offer, "wrong-build").err(),
        Some(WorksetRefusal::Host)
    );
}

#[test]
fn four_plots_reserve_exact_input_deliveries_from_one_initialized_keyboard() {
    let (ids, offer) = fixture();
    let wake = wake(&inventory());
    let prepared = prepare(&wake, &ids, &offer, "build").unwrap();
    assert_eq!(prepared.lowered.nodes, 10);
    assert_eq!(prepared.lowered.cords, 8);
    assert_eq!(prepared.plan.plots.len(), 4);
    assert_eq!(prepared.keyboard, offer.keyboard.unwrap().realization);
    assert_eq!(prepared.input_owners().len(), 4);
    for (plot, owner) in prepared.plan.plots.iter().zip(prepared.input_owners()) {
        let source = plot.plan.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                matches!(
                    placement.kind_id.as_str(),
                    conduit_semantic_catalog::KEYBOARD_KIND
                        | conduit_semantic_catalog::APPLICATION_EVENT_SOURCE_KIND
                        | conduit_semantic_catalog::FACE_EDITOR_KIND
                        | patchbay_application::PATCHBAY_INTERACTION_KIND
                )
            })
            .unwrap();
        assert_eq!(owner.plot, plot.plot);
        assert_eq!(owner.placement_id, source.placement_id);
        if source.kind_id.as_str() == conduit_semantic_catalog::KEYBOARD_KIND {
            assert_eq!(
                owner.kind_id.as_str(),
                conduit_semantic_catalog::KEYBOARD_KIND
            );
            assert_eq!(
                owner.port_id.as_str(),
                conduit_semantic_catalog::KEYBOARD_PORT
            );
            assert_eq!(owner.value_kind.as_str(), conduit_human::KEY_EVENT_INFO_ID);
            assert_eq!(
                source.implementation_id.as_str(),
                keyboard_delivery::IMPLEMENTATION
            );
            assert!(
                source
                    .resources
                    .iter()
                    .any(
                        |resource| resource.class_id.as_str() == keyboard_delivery::RESOURCE
                            && resource.units == 1
                    )
            );
            assert!(!source.resources.iter().any(
                |resource| resource.class_id.as_str() == crate::keyboard_offer::DEVICE_RESOURCE
            ));
        } else if source.kind_id.as_str() == conduit_semantic_catalog::FACE_EDITOR_KIND {
            assert_eq!(owner.port_id.as_str(), "edit");
            assert_eq!(
                owner.value_kind.as_str(),
                conduit_semantic_catalog::TEXT_EDIT_VALUE_KIND
            );
            assert_eq!(
                source.implementation_id.as_str(),
                editor_delivery::IMPLEMENTATION
            );
            assert!(
                source
                    .resources
                    .iter()
                    .any(
                        |resource| resource.class_id.as_str() == keyboard_delivery::RESOURCE
                            && resource.units == 1
                    )
            );
        } else if source.kind_id.as_str() == patchbay_application::PATCHBAY_INTERACTION_KIND {
            assert_eq!(owner.port_id.as_str(), "interaction");
            assert_eq!(
                owner.value_kind.as_str(),
                conduit_face::FACE_INTERACTION_VALUE_KIND
            );
            assert_eq!(
                source.implementation_id.as_str(),
                patchbay_ingress::IMPLEMENTATION
            );
        } else {
            assert_eq!(
                owner.kind_id.as_str(),
                conduit_semantic_catalog::APPLICATION_EVENT_SOURCE_KIND
            );
            assert_eq!(
                owner.port_id.as_str(),
                conduit_semantic_catalog::APPLICATION_EVENT_PORT
            );
            assert_eq!(
                owner.value_kind.as_str(),
                conduit_face::APPLICATION_EVENT_INFO_ID
            );
            assert_eq!(
                source.implementation_id.as_str(),
                application_delivery::EVENT_IMPLEMENTATION
            );
        }
    }
    assert!(
        prepared
            .advertisement
            .resources
            .iter()
            .filter(|resource| resource.class_id.as_str() == crate::keyboard_offer::DEVICE_RESOURCE)
            .all(|resource| resource.capacity_units == 1)
    );
}

#[test]
fn pre_birth_review_checks_each_exact_plot_without_a_body_or_play() {
    let (ids, mut offer) = fixture();
    for plot in inventory() {
        assert_eq!(review(plot, &ids, &offer, "build"), Ok(()));
    }
    offer.keyboard = None;
    for plot in inventory() {
        assert_eq!(
            review(plot, &ids, &offer, "build"),
            Err(WorksetRefusal::Host)
        );
    }
}

#[test]
fn visible_checked_candidate_does_not_widen_installed_workset_authority() {
    let candidates = candidate_inventory();
    assert_eq!(candidates.len(), 5);
    assert_eq!(&candidates[..4], &inventory());
    assert_eq!(profile().capacity, 4);
    let hello = resident(NativePlot::Hello).unwrap();
    assert_eq!(
        NativePlot::Hello.source(),
        include_str!("../../../../plots/hello/main.conduit")
    );
    assert!(!profile().contains(&hello).unwrap());
    assert_eq!(resolve(&hello), Err(WorksetRefusal::UnknownPlot));
    let (ids, offer) = fixture();
    assert_eq!(
        review(NativePlot::Hello, &ids, &offer, "build"),
        Err(WorksetRefusal::UnknownPlot)
    );
    assert!(matches!(
        prepare(&wake(&[NativePlot::Hello]), &ids, &offer, "build"),
        Err(WorksetRefusal::UnknownPlot)
    ));
}

#[test]
fn compact_keyboard_target_retains_exact_graph_and_canonical_values() {
    use conduitos_application_core::data::{self, Command, Target};
    use sha2::{Digest, Sha256};
    let expanded = super::checked(super::NativePlot::KeyboardCanvas).unwrap();
    let graph = patchbay_graph::PatchbayGraph::from_expanded(&expanded).unwrap();
    assert_eq!(
        graph.source_document_id.as_str(),
        "fd681a367856550384d5155c8ba3e7f36c6c008c249029b009aa075ac6c23516"
    );
    assert_eq!(
        graph.checked_plot_id.as_str(),
        "f70ecce1b064835918a9ff13e0e5665fe5d50005ac5ab1155cfc75b9e606732b"
    );
    assert_eq!(
        graph.expanded_plot_id.as_str(),
        "bb0bfd56d97029a496e5b8652134f0c76b3b460fb1de828cc6ef5018a5dcf2f1"
    );
    let command = Command::PrepareTarget {
        slot: 2,
        target: 3,
        value: Target {
            graph,
            plan: Some("65f2a3cf560c587fba263ebe699211d24fa359b871d68015e8d6f8008237f3dd".into()),
            body:
                "body-plan/sha256:5057f696991a44d70135381a3320929c0e47b40539734d51f3f876a64d829945"
                    .into(),
        },
    };
    let bytes = data::encode_command(&command).unwrap();
    println!(
        "COMPACT_KEYBOARD_TARGET wire_bytes={} decoded_bytes={} sha256={:x}",
        bytes.len(),
        u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        Sha256::digest(&bytes)
    );
    assert_eq!(bytes.len(), 9291, "exact retained CI input extent");
    assert_eq!(
        alloc::format!("{:x}", Sha256::digest(&bytes)),
        "1ab6ae2e4cdec601570355f04e4dbe04ca7188fdafa540cd6ca2e086125d5ea5",
        "actual failing CI command, not a merely related fixture"
    );
    if let Some(path) = std::env::var_os("CONDUIT_PREPARE_TARGET_CAPTURE") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("command.bin"), &bytes).unwrap();
        let Command::PrepareTarget { value, .. } = &command else {
            unreachable!()
        };
        std::fs::write(
            path.join("graph.json"),
            serde_json::to_vec(&value.graph).unwrap(),
        )
        .unwrap();
    }
    // The exact former 17,606-byte / 426,638-expanded-byte payload and its
    // f92c0894... SHA-256 are retained at d1a15a3e3 and in the native receipt.
    // V2 changes only the private byte-array representation. Keep the same
    // complete target and assert its existing wire/expanded bounds directly.
    assert!(bytes.len() <= conduitos_application_core::MAX_BLOB);
    assert_eq!(&bytes[..8], b"CAPZ\x01\0\0\0");
    let expanded_bytes = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    assert!(expanded_bytes < 426_638);
    let mut workspace = data::Workspace::new().unwrap();
    let started = std::time::Instant::now();
    let decoded = workspace.decode_command(&bytes).unwrap();
    let elapsed = started.elapsed();
    let Command::PrepareTarget {
        slot,
        target,
        value,
    } = decoded
    else {
        panic!("exact command variant");
    };
    assert_eq!((slot, target), (2, 3));
    let Command::PrepareTarget {
        value: original, ..
    } = &command
    else {
        unreachable!()
    };
    assert_eq!(value.graph, original.graph);
    assert_eq!(value.plan, original.plan);
    assert_eq!(value.body, original.body);
    println!(
        "HOST_DIAGNOSTIC full_public_decode_ns={} includes_decompress_preflight_typed_decode=true guest_budget_proof=false",
        elapsed.as_nanos()
    );
    for gear in &value.graph.gears {
        for control in &gear.controls {
            if let conduit_core::ConfigurationValue::Structured(structured) = &control.value {
                let profile = structured.profile().clone();
                let canonical = structured.canonical_value().to_vec();
                let length = canonical.len();
                let started = std::time::Instant::now();
                let rebuilt = conduit_core::StructuredConfigurationValue::new(profile, canonical)
                    .expect("same exact canonical value must retain validation");
                let elapsed = started.elapsed();
                assert_eq!(&rebuilt, structured);
                println!(
                    "HOST_DIAGNOSTIC gear={} control={} canonical_bytes={} structured_validation_ns={} guest_budget_proof=false",
                    gear.gear_id.as_str(),
                    control.key,
                    length,
                    elapsed.as_nanos()
                );
            }
        }
    }
}
