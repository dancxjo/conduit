//! Exact canonical bytes only; digest inputs confer no approval.
use conduit_core::*;
#[path = "common/sealed_state.rs"]
mod fixture;
fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[test]
fn fresh_fragment_commitments_match_full_core_sealing_for_two_boots() {
    let mut ids = Vec::new();
    for boot in ["boot/first", "boot/second"] {
        let mut fragment = fixture::fragment();
        fragment.boot_id = boot.into();
        for p in &mut fragment.placements {
            p.boot_id = boot.into();
        }
        let plan = seal_plan(
            PlotIdentity {
                source_document_id: "source".into(),
                checked_plot_id: "checked".into(),
                expanded_plot_id: "expanded".into(),
            },
            vec![fragment],
        );
        let pins = PlainPlotPlanSeal::capture(&plan).unwrap();
        let f = &plan.fragments[0];
        let digest = pins
            .digest(&[PlotFragmentSeal {
                host_id: f.host_id.as_str(),
                fragment_id: f.fragment_id.as_str(),
            }])
            .unwrap();
        assert_eq!(hex(digest), plan.plan_id.as_str());
        ids.push(digest);
        assert_eq!(pins.digest(&[]), Err(PlotPlanSealRefusal::Commitments));
        let row = PlotFragmentSeal {
            host_id: "h",
            fragment_id: "f",
        };
        assert_eq!(
            pins.digest(&[row, row]),
            Err(PlotPlanSealRefusal::Commitments)
        );
        let mut unsupported = plan.clone();
        // A nonempty activation-preparation tail must never silently disappear.
        unsupported
            .activation_preparations
            .push(PlannedActivationPreparationBinding {
                activation_id: "a".into(),
                owner_placement_id: "p".into(),
                owner_fragment_id: "f".into(),
                owner_host_id: "h".into(),
                owner_boot_id: "b".into(),
                owner_offer_generation: OfferGeneration(1),
                selected_plan_id: "s".into(),
                child_fragments: vec![],
            });
        assert!(matches!(
            PlainPlotPlanSeal::capture(&unsupported),
            Err(PlotPlanSealRefusal::Unsupported)
        ));
    }
    assert_ne!(ids[0], ids[1]);
}
