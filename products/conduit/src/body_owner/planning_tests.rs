//! Complete Source custody and atomic preparation through the installed owner.
use super::*;
use conduit_std_host::{StdHost, StdHostConfig};

fn source(name: &str) -> crate::plot_source::CanonicalSource {
    crate::plot_source::parse(&format!(
        "plot {name} {{\n show: face/text(\"Message\")\n \"{name}\" >> show\n}}."
    ))
    .unwrap()
}
fn resident(plot: &conduit_plot::ExpandedAuthoringPlot) -> ResidentPlot {
    ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    )
}
fn owner(
    a: &conduit_plot::ExpandedAuthoringPlot,
    b: &conduit_plot::ExpandedAuthoringPlot,
) -> Owner {
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/workset-plan-test".into(),
        boot_id: "boot/workset-plan-test".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let ad = host.advertisement().clone();
    let mut owner = Owner::open(host, resident(a), None, "Two Plots").unwrap();
    // Fixture establishes an already-retained workset through the existing
    // lifecycle. This is not a proposal or an authorized adoption demonstration.
    owner
        .session
        .admit_plot(0, resident(b), &ad.host_id, &ad.boot_id)
        .unwrap();
    owner
}
#[test]
fn complete_checked_workset_uses_one_body_plan_and_actual_kernel_play() {
    let a = source("first");
    let b = source("second");
    let pa = a.expand_entry_for_authoring().unwrap();
    let pb = b.expand_entry_for_authoring().unwrap();
    let mut owner = owner(&pa, &pb);
    let before = owner.session.evidence().clone();
    owner
        .plan_workset_with_sources(&[(&b, &pb), (&a, &pa)])
        .unwrap();
    let sealed = owner.session.realization().unwrap().plan.clone();
    let proposal = owner.session.realization().unwrap();
    let prospective = owner
        .host
        .prospective_body_play(&proposal.wake, &sealed)
        .unwrap();
    assert_eq!(
        owner
            .host
            .prospective_body_play(&proposal.wake, &sealed)
            .unwrap(),
        prospective
    );
    assert_eq!(sealed.plots.len(), 2);
    assert!(owner.session.realization().unwrap().play.is_none());
    assert!(owner.last_execution.is_none());
    assert_eq!(owner.session.evidence().body_id, before.body_id);
    assert_eq!(owner.session.evidence().membership, before.membership);
    assert_eq!(owner.session.evidence().body.workset, before.body.workset);
    assert!(owner
        .plan_workset_with_sources(&[(&a, &pa), (&b, &pb)])
        .is_err());
    assert_eq!(owner.session.realization().unwrap().plan, sealed);
    owner.execute_reference_fixture(1_000).unwrap();
    let actual = owner.last_execution.as_ref().unwrap();
    assert_eq!(actual["play"], serde_json::to_value(&prospective).unwrap());
    assert_eq!(
        actual["terminal"],
        serde_json::json!(conduit_core::TerminalDisposition::Completed)
    );
    assert!(actual["play"]["active_play_id"].is_string());
    assert_eq!(actual["play"]["plan_id"], serde_json::json!(sealed.plan_id));
    assert_eq!(actual["play"]["body_id"], serde_json::json!(before.body_id));
    assert!(actual["cleanup_failure"].is_null());
    let output = actual["output_utf8"].as_str().unwrap();
    assert!(output.contains("first"), "{output}");
    assert!(output.contains("second"), "{output}");
    assert!(owner.session.realization().is_none());
    assert_eq!(owner.session.evidence().body.state, BodyState::Lulled);
    assert_eq!(owner.session.evidence().body.workset, before.body.workset);
    assert_eq!(owner.session.evidence().membership, before.membership);
}
#[test]
fn incomplete_duplicate_or_forged_source_preparation_preserves_idle_owner() {
    let a = source("first");
    let b = source("second");
    let pa = a.expand_entry_for_authoring().unwrap();
    let pb = b.expand_entry_for_authoring().unwrap();
    let mut owner = owner(&pa, &pb);
    let before = owner.session.evidence().clone();
    let mut forged = pb.clone();
    forged.expanded.name = "forged inspection".into();
    for sources in [
        vec![(&a, &pa)],
        vec![(&a, &pa), (&a, &pa)],
        vec![(&a, &pa), (&a, &pb)],
        vec![(&a, &pa), (&b, &forged)],
    ] {
        assert!(owner.plan_workset_with_sources(&sources).is_err());
        assert!(owner
            .plan_workset_with_source_boundaries_and_provider(
                &sources,
                &BTreeMap::new(),
                &mut |_| panic!("native provider must not see unchecked or incomplete Source"),
            )
            .is_err());
        assert_eq!(owner.session.evidence(), &before);
        assert!(owner.session.realization().is_none());
        assert!(owner.last_execution.is_none());
    }
}

#[test]
fn later_partition_refusal_never_publishes_a_partial_wake() {
    let a = source("first");
    let unsupported =
        crate::plot_source::parse(include_str!("../../../../plots/todo/live.conduit")).unwrap();
    let pa = a.expand_entry_for_authoring().unwrap();
    let pb = unsupported.expand_entry_for_authoring().unwrap();
    let mut owner = owner(&pa, &pb);
    let before = owner.session.evidence().clone();
    // The first partition really can be planned on this Host. The second
    // needs a scan provider absent from this ordinary installed Host profile.
    assert!(owner.plan_partition(&pa, &resident(&pa)).is_ok());
    assert!(owner
        .plan_workset_with_sources(&[(&a, &pa), (&unsupported, &pb)])
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert!(owner.session.realization().is_none());
    assert!(owner.last_execution.is_none());
    let oversized = vec![(&a, &pa); conduit_body::MAX_BODY_PLOTS + 1];
    assert!(owner.plan_workset_with_sources(&oversized).is_err());
    assert_eq!(owner.session.evidence(), &before);
}
