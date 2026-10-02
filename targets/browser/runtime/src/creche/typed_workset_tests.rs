//! Reviewed presentation profiles select existing exact typed realizations in one body.
use super::{initial_plots, session};

fn bundle() -> String {
    serde_json::json!({
        "schema": "conduit.creche/reviewed-plot-bundle@2",
        "plots": [
            {"slug": "memory-lantern", "entry": "memory_lantern", "source": include_str!("../../../../../plots/memory-lantern/main.conduit")},
            {"slug": "secret-knock", "entry": "secret-knock-demo", "presentation_profile": 3, "source": include_str!("../../../../../plots/secret-knock/main.conduit")}
        ]
    }).to_string()
}

#[test]
fn canonical_theremin_checks_without_inventing_a_browser_state_back() {
    let source = serde_json::json!({
        "schema": "conduit.creche/reviewed-plot-bundle@2",
        "plots": [{
            "slug": "pocket-theremin",
            "entry": "pocket-theremin",
            "presentation_profile": 1,
            "source": include_str!("../../../../../plots/pocket-theremin/main.conduit"),
        }],
    })
    .to_string();
    let inventory = initial_plots::reviewed_inventory(&source).unwrap();
    assert_eq!(inventory.plots.len(), 1);
    assert!(!inventory.plots[0]
        .required_kinds
        .contains(&"state/latest".to_owned()));
    assert!(inventory.plots[0]
        .required_kinds
        .contains(&"audio/continuous-tone".to_owned()));
    assert!(inventory.plots[0]
        .required_kinds
        .contains(&"audio/apply-gain".to_owned()));
    let host =
        initial_plots::reviewed_browser_host(&source, "host/typed".into(), "boot/typed".into())
            .unwrap();
    assert!(!host
        .capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str() == "state/latest"));
}

#[test]
fn phone_theremin_checks_with_exact_two_axis_browser_realizations() {
    let source = serde_json::json!({
        "schema": "conduit.creche/reviewed-plot-bundle@2",
        "plots": [{
            "slug": "pocket-theremin",
            "entry": "phone-two-axis-controller",
            "presentation_profile": 1,
            "source": include_str!("../../../../../plots/pocket-theremin/main.conduit"),
        }],
    })
    .to_string();
    let inventory = initial_plots::reviewed_inventory(&source).unwrap();
    let required = &inventory.plots[0].required_kinds;
    for kind in [
        "input/pointer-source",
        "math/map-normalized-distance",
        "audio/play",
        "pocket-theremin",
    ] {
        assert!(required.iter().any(|required| required == kind), "{kind}");
    }
    let host = initial_plots::reviewed_browser_host(
        &source,
        "host/theremin".into(),
        "boot/theremin".into(),
    )
    .unwrap();
    for implementation in [
        "browser/plot-pointer-source@1",
        "browser/kernel-audio-tone@1",
        "browser/kernel-audio-apply-gain@1",
        "browser/kernel-map-distance-frequency@1",
    ] {
        assert!(
            host.capabilities
                .iter()
                .any(|offer| { offer.implementation.implementation_id.as_str() == implementation }),
            "{implementation}"
        );
    }
    let gain = host
        .capabilities
        .iter()
        .find(|offer| {
            offer.implementation.implementation_id.as_str() == "browser/kernel-audio-apply-gain@1"
        })
        .unwrap();
    assert!(gain
        .host_calls
        .windows(2)
        .all(|pair| pair[0].contract_id < pair[1].contract_id));
}

#[test]
fn typed_inventory_accepts_canonical_default_aliases() {
    let source = bundle();
    let entries = initial_plots::reviewed_inventory(&source).unwrap();
    let selection: Vec<_> = entries
        .plots
        .iter()
        .map(|entry| initial_plots::InitialPlotSelection {
            name: entry.name.clone(),
            source_document_id: entry.source_document_id.clone(),
            checked_plot_id: entry.checked_plot_id.clone(),
        })
        .collect();
    let host =
        initial_plots::reviewed_browser_host(&source, "host/typed".into(), "boot/typed".into())
            .unwrap();
    assert!(super::review::review(
        &source,
        &serde_json::to_string(&selection).unwrap(),
        &[host],
        &crate::installed_browser::local_bases()
    )
    .is_ok());
    let inventory = initial_plots::reviewed_inventory(&source).unwrap();
    assert_eq!(inventory.plots.len(), 2);
    assert!(inventory
        .plots
        .iter()
        .all(|entry| !entry.required_kinds.is_empty()));
}

#[test]
fn text_and_pattern_presentations_plan_together_under_one_body() {
    session::clear_for_test();
    let source = bundle();
    let inventory = initial_plots::reviewed_inventory(&source).unwrap();
    let selected: Vec<_> = inventory
        .plots
        .iter()
        .map(|plot| initial_plots::InitialPlotSelection {
            name: plot.name.clone(),
            source_document_id: plot.source_document_id.clone(),
            checked_plot_id: plot.checked_plot_id.clone(),
        })
        .collect();
    let interaction = crate::source_interaction::admit_source(source.as_bytes(), 2260).unwrap();
    let receipt = session::birth(
        "browser/typed-workset",
        "boot/typed-workset",
        "Many voices",
        &serde_json::to_string(&selected).unwrap(),
        &source,
        2260,
        interaction,
    )
    .unwrap();
    assert_eq!(receipt.initial_plots.len(), 2);
    #[cfg(feature = "plot-runner")]
    {
        let observed = super::initial_plots::reviewed_browser_host(
            &source,
            "browser/typed-workset".into(),
            "boot/typed-workset".into(),
        )
        .unwrap();
        let plans = super::workspace::plan_workspace_plots(
            &session::biography().unwrap(),
            &source,
            &[observed],
            &"browser/typed-workset".into(),
            &"boot/typed-workset".into(),
            &[],
            super::PlanningAuthority::default(),
        )
        .unwrap();
        assert_eq!(plans.len(), 2);
        let selector = plans
            .iter()
            .flat_map(|part| &part.plan.fragments)
            .flat_map(|fragment| &fragment.placements)
            .find(|gear| {
                gear.implementation_id.as_str()
                    == crate::installed_browser::structured_selector::IMPLEMENTATION
            })
            .unwrap();
        let installed =
            crate::installed_browser::structured_selector::offer_for_placement(selector)
                .unwrap()
                .unwrap();
        assert_eq!(installed.capability_id, selector.capability_id);
        let mut substituted = selector.clone();
        substituted.capability_id = "selector/substituted".into();
        assert!(
            crate::installed_browser::structured_selector::offer_for_placement(&substituted)
                .is_err()
        );
        substituted = selector.clone();
        substituted.outputs.clear();
        assert!(
            crate::installed_browser::structured_selector::offer_for_placement(&substituted)
                .is_err()
        );
        substituted = selector.clone();
        substituted.configuration.clear();
        assert!(
            crate::installed_browser::structured_selector::offer_for_placement(&substituted)
                .is_err()
        );

        for (plan, plot) in plans.iter().zip(receipt.raw_body.workset.plots()) {
            assert_eq!(&plan.plot, plot);
            assert!(plan
                .plan
                .fragments
                .iter()
                .all(|fragment| fragment.host_id.as_str() == "browser/typed-workset"));
        }
    }
    session::clear_for_test();
}
