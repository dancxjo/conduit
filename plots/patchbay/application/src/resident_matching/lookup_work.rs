//! Test-only accounting for the actual checkpointed matcher's planned lookup.
extern crate std;
use core::cell::Cell;

std::thread_local! {
    static WORK: Cell<[usize; 4]> = const { Cell::new([0; 4]) };
}
pub(super) fn query(identity: &str) {
    WORK.with(|work| {
        let mut counts = work.get();
        counts[0] += 1;
        counts[1] += usize::from(!identity.starts_with("plot/"));
        work.set(counts);
    });
}
pub(super) fn entry(identity: &str) {
    WORK.with(|work| {
        let mut counts = work.get();
        counts[2] += 1;
        counts[3] += usize::from(!identity.starts_with("plot/"));
        work.set(counts);
    });
}

#[test]
fn actual_checkpointed_matcher_never_scans_plots_for_non_plot_identities() {
    use super::*;
    use conduit_core::{ActivePlayId, CheckedPlotId, PlanId};
    let plans = conduit_patchbay_workbench_conformance::patchbay_mask_plans().unwrap();
    let mut port = PatchbayApplicationPort::open(
        &plans.direct_expanded,
        plans.direct.plan_id,
        PlanId::from("body-plan/lookup-work"),
    )
    .unwrap();
    port.select_prepared_plot(0).unwrap();
    let body: BodyId = serde_json::from_str("\"body/lookup-work\"").unwrap();
    let basis = FaceContributionBasis {
        checked_plot_id: CheckedPlotId::from("checked/lookup-work"),
        plan_id: PlanId::from("plan/lookup-work"),
        active_play_id: ActivePlayId::from("play/lookup-work"),
        required_interaction_context: None,
    };
    WORK.with(|work| work.set([0; 4]));
    // This is the same check_contribution path as publication matching, without
    // a received Face index. All actual subject/disclosure checkpoints run.
    port.validate_projection_bounds(basis, &body).unwrap();
    let [queries, non_plot_queries, entries, non_plot_entries] = WORK.with(Cell::get);
    std::eprintln!("planned lookup queries={queries} non_plot_queries={non_plot_queries} entries={entries} non_plot_entries={non_plot_entries}");
    assert!(
        non_plot_queries > 1,
        "retained non-plot subjects were exercised"
    );
    assert!(queries > non_plot_queries, "plot lookups were exercised");
    assert!(entries > 0, "planned plot matching was exercised");
    assert_eq!(non_plot_entries, 0);
    // With this one-entry inventory, the former prefix-inside-scan algorithm
    // visits an entry for every query, including every retained non-plot ID.
    assert_eq!(entries, queries - non_plot_queries);
}
