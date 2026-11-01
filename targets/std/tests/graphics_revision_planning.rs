//! Exercise real portable contracts and installed offers at the public planner
//! entrances. These are planning proofs, not Host Call or device execution proof.
use conduit_core::{kind_id, KindIdentity};
use conduit_planner::{default_placements, plan, PlacementChoice, PlacementChoices};
use conduit_plot::ProfileCatalog;
use conduit_semantic_catalog::{
    graphics_face_contract, graphics_icon_contract, graphics_rect_contract, graphics_text_contract,
    StandardKindContract, GRAPHICS_FACE_REVISION, GRAPHICS_SCENE_CONTRACT_REVISION,
};
use conduit_std_host::StdHost;

fn revision(terminal: bool, current: bool) -> &'static str {
    match (terminal, current) {
        (false, false) => "conduit.std/graphics-scene@1",
        (true, false) => "conduit.std/face-graphics@1",
        (false, true) => GRAPHICS_SCENE_CONTRACT_REVISION,
        (true, true) => GRAPHICS_FACE_REVISION,
    }
}
// Reconstruct the legacy domain with its original finite value envelopes.
// The historical factory is not reused: it now owns only the current domain.
fn legacy_ports_and_laws(
    ports: &mut [conduit_core::PortDescriptor],
    laws: &mut [conduit_core::KindSemanticLaw],
) {
    for port in ports {
        if port.value_kind.as_str() == "face/graphics-scene@2" {
            port.value_kind = kind_id("face/graphics-scene@1");
        }
    }
    for law in laws {
        if let conduit_core::KindSemanticLaw::ValueContracts(values) = law {
            for value in values {
                if value.contract.value_kind.as_str() == "face/graphics-scene@2" {
                    value.contract.value_kind = kind_id("face/graphics-scene@1");
                }
            }
        }
    }
}
fn portable(contract: StandardKindContract, current: bool) -> conduit_core::Kind {
    let terminal = contract.kind_id.as_str() == "face/graphics";
    let mut kind = contract.into_semantic_contract(revision(terminal, true));
    if !current {
        kind.kind_contract_revision = KindIdentity::from(revision(terminal, false));
        legacy_ports_and_laws(&mut kind.inputs, &mut kind.semantic_laws);
        legacy_ports_and_laws(&mut kind.outputs, &mut kind.semantic_laws);
    }
    kind
}

#[test]
fn old_and_union_contracts_plan_only_against_the_matching_installed_offers() {
    assert_eq!(
        GRAPHICS_SCENE_CONTRACT_REVISION,
        "conduit.std/graphics-scene@2"
    );
    assert_eq!(GRAPHICS_FACE_REVISION, "conduit.std/face-graphics@2");
    for contract in [
        graphics_rect_contract(),
        graphics_text_contract(),
        graphics_icon_contract(),
        graphics_face_contract(),
    ] {
        let kind = contract.kind_id.clone();
        let terminal = kind.as_str() == "face/graphics";
        let configuration = if terminal { "(\"Graphics\")" } else { "" };
        for authored_current in [false, true] {
            let mut catalog = ProfileCatalog::new();
            catalog
                .insert_kind(portable(contract.clone(), authored_current))
                .unwrap();
            let checked = conduit_plot::parse(
                &format!(
                    "plot graphics {{\n leaf: {}{}\n}}\n",
                    kind.as_str(),
                    configuration
                ),
                &catalog,
            )
            .unwrap();
            let mut startup = conduit_plot::StartupCatalog::new();
            let mut unused_profile = ProfileCatalog::new();
            conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut unused_profile)
                .unwrap();
            conduit_semantic_catalog::install_graphics_face_catalog(
                &mut startup,
                &mut unused_profile,
            )
            .unwrap();
            let syntax = conduit_plot::parse_syntax_document(&format!(
                "plot graphics {{\n leaf: {}{}\n}}\n",
                kind.as_str(),
                configuration
            ));
            let canonical = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
            let expanded =
                conduit_plot::expand_canonical_plot(&canonical, "graphics", &catalog).unwrap();
            for offered_current in [false, true] {
                // Preserve installed resources, authority and implementation facts.
                let installed = StdHost::new();
                let mut host = installed.advertisement().clone();
                host.capabilities.retain(|offer| offer.kind_id == kind);
                assert!(!host.capabilities.is_empty());
                for offer in &mut host.capabilities {
                    assert_eq!(
                        offer.kind_contract_revision.as_str(),
                        revision(terminal, true)
                    );
                    if !offered_current {
                        offer.kind_contract_revision =
                            KindIdentity::from(revision(terminal, false));
                        legacy_ports_and_laws(&mut offer.inputs, &mut offer.semantic_contract.laws);
                        legacy_ports_and_laws(
                            &mut offer.outputs,
                            &mut offer.semantic_contract.laws,
                        );
                    }
                }
                let expected = authored_current == offered_current;
                let hosts = [host];
                let automatic = default_placements(&checked, &hosts);
                assert_eq!(
                    automatic.is_ok(),
                    expected,
                    "default {kind:?}: {authored_current}/{offered_current}: {automatic:?}"
                );
                if let Ok(choices) = automatic {
                    assert!(plan(&checked, &hosts, &choices, &[]).is_ok());
                }
                let expanded_choices =
                    conduit_planner::default_expanded_placements(&expanded, &hosts);
                assert_eq!(expanded_choices.is_ok(), expected, "expanded default {kind:?}: {authored_current}/{offered_current}: {expanded_choices:?}");
                if let Ok(choices) = expanded_choices {
                    assert!(conduit_planner::plan_expanded_canonical(
                        &expanded,
                        &hosts,
                        &choices,
                        &[]
                    )
                    .is_ok());
                }
                let offer = &hosts[0].capabilities[0];
                let forced = PlacementChoices {
                    by_gear: [(
                        checked.gears[0].gear_id.clone(),
                        PlacementChoice {
                            host_id: hosts[0].host_id.clone(),
                            capability_id: offer.capability_id.clone(),
                        },
                    )]
                    .into_iter()
                    .collect(),
                };
                let expanded_selected =
                    conduit_planner::plan_expanded_canonical(&expanded, &hosts, &forced, &[]);
                assert_eq!(expanded_selected.is_ok(), expected, "expanded selected {kind:?}: {authored_current}/{offered_current}: {expanded_selected:?}");
                let selected = plan(&checked, &hosts, &forced, &[]);
                assert_eq!(
                    selected.is_ok(),
                    expected,
                    "selected {kind:?}: {authored_current}/{offered_current}: {selected:?}"
                );
            }
        }
    }
}
