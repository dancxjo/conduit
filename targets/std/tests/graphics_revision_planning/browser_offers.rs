//! Actual browser transform offer factory, on a synthetic planning-only carrier.
//! No invented browser graphics terminal, installed browser or Host execution claim.
use super::*;
use conduit_core::{HostAdvertisement, OfferGeneration, PROTOCOL_VERSION};

fn carrier() -> HostAdvertisement {
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "browser/graphics-domain-proof".into(),
        boot_id: "boot/graphics-domain-proof".into(),
        offer_generation: OfferGeneration(1),
        profile: conduit_browser_runtime::face_nucleus::BROWSER_FACE_PROFILE.into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: conduit_browser_runtime::face_nucleus::offers(),
    }
}

#[test]
fn actual_browser_transform_offers_obey_both_graphics_revision_directions() {
    assert!(!carrier()
        .capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str() == "face/graphics"));
    let mut decisions = 0;
    for contract in [
        graphics_rect_contract(),
        graphics_text_contract(),
        graphics_icon_contract(),
    ] {
        for authored_current in [false, true] {
            let mut catalog = ProfileCatalog::new();
            catalog
                .insert_kind(portable(contract.clone(), authored_current))
                .unwrap();
            let source = format!(
                "plot browser-graphics {{\n leaf: {}\n}}\n",
                contract.kind_id.as_str()
            );
            let checked = conduit_plot::parse(&source, &catalog).unwrap();
            let mut startup = conduit_plot::StartupCatalog::new();
            let mut unused = ProfileCatalog::new();
            conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut unused).unwrap();
            let syntax = conduit_plot::parse_syntax_document(&source);
            let canonical = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
            let expanded =
                conduit_plot::expand_canonical_plot(&canonical, "browser-graphics", &catalog)
                    .unwrap();
            for offered_current in [false, true] {
                let mut host = carrier();
                host.capabilities
                    .retain(|offer| offer.kind_id == contract.kind_id);
                assert_eq!(host.capabilities.len(), 1);
                let offer = &mut host.capabilities[0];
                assert_eq!(
                    offer.kind_contract_revision.as_str(),
                    GRAPHICS_SCENE_CONTRACT_REVISION
                );
                if !offered_current {
                    offer.kind_contract_revision = revision(false, false).into();
                    legacy_ports_and_laws(&mut offer.inputs, &mut offer.semantic_contract.laws);
                    legacy_ports_and_laws(&mut offer.outputs, &mut offer.semantic_contract.laws);
                }
                let forced = PlacementChoices {
                    by_gear: [(
                        checked.gears[0].gear_id.clone(),
                        PlacementChoice {
                            host_id: host.host_id.clone(),
                            capability_id: offer.capability_id.clone(),
                        },
                    )]
                    .into_iter()
                    .collect(),
                };
                let hosts = [host];
                let expected = authored_current == offered_current;
                let automatic = default_placements(&checked, &hosts);
                assert_eq!(
                    automatic.is_ok(),
                    expected,
                    "checked default {:?}: {authored_current}/{offered_current}",
                    contract.kind_id
                );
                if let Ok(choices) = automatic {
                    assert!(plan(&checked, &hosts, &choices, &[]).is_ok());
                }
                assert_eq!(plan(&checked, &hosts, &forced, &[]).is_ok(), expected);
                let automatic = conduit_planner::default_expanded_placements(&expanded, &hosts);
                assert_eq!(automatic.is_ok(), expected);
                if let Ok(choices) = automatic {
                    assert!(conduit_planner::plan_expanded_canonical(
                        &expanded,
                        &hosts,
                        &choices,
                        &[]
                    )
                    .is_ok());
                }
                assert_eq!(
                    conduit_planner::plan_expanded_canonical(&expanded, &hosts, &forced, &[])
                        .is_ok(),
                    expected
                );
                decisions += 4;
            }
        }
    }
    assert_eq!(decisions, 48);
}
