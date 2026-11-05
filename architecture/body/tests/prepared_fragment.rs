//! A borrowed scope preserves exact containing Body identity without authority.
use conduit_body::*;
use conduit_core::*;
#[path = "common/admission_allocation_probe.rs"]
mod probe;
#[path = "prepared_fragment/support.rs"]
mod support;
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;

fn resident(plan: &Plan) -> ResidentPlot {
    ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    )
}
fn fixture(mask: Option<&Plan>) -> (Wake, BodyPlan, BodyPlayIdentity) {
    let a = support::plan("first", "host/local");
    let b = support::plan("second", "host/local");
    let c = support::plan("remote", "host/remote");
    let partitions: Vec<_> = [a, b, c]
        .into_iter()
        .map(|plan| BodyPlotPlan {
            plot: resident(&plan),
            plan,
        })
        .collect();
    let workset = BodyWorkset::from_plots(partitions.iter().map(|p| p.plot.clone())).unwrap();
    let body = Body::born_with_plots(workset, 1, "born".into()).unwrap();
    let wake = body.wake(1, "woke".into()).unwrap().1;
    let masks = mask
        .map(|plan| BodyMaskTopology {
            face: BodyFaceSelector {
                plot: None,
                source_placement_id: None,
            },
            chains: vec![BodyMaskChainPlan {
                plan: plan.clone(),
                stage_placement_ids: vec![plan.fragments[0].placements[0].placement_id.clone()],
            }],
        })
        .into_iter()
        .collect();
    let plan = BodyPlan::seal_with_masks(&wake, partitions, masks).unwrap();
    let play = BodyPlayIdentity::bind(&plan, 1);
    (wake, plan, play)
}

#[test]
fn complete_local_set_includes_masks_and_borrows_original_plan_without_allocations() {
    let mask = support::plan("mask", "host/local");
    let (wake, plan, play) = fixture(Some(&mask));
    let anchor = &plan.plots[0].plan.fragments[0];
    let scope = PreparedBodyFragment::prepare(&wake, &plan, &play, anchor).unwrap();
    let refusal = probe::refuse_next_allocation_size(1);
    let (_, cost) = probe::observe(|| {
        let mut count = 0;
        let mut masks = 0;
        for local in scope.local_fragments() {
            let local = local.unwrap();
            assert!(core::ptr::eq(local.plan(), &plan));
            assert!(core::ptr::eq(local.play(), &play));
            assert_eq!(local.fragment().host_id.as_str(), "host/local");
            assert!(local
                .plot_plan()
                .fragments
                .iter()
                .any(|f| core::ptr::eq(f, local.fragment())));
            masks += usize::from(local.plot_plan().plan_id == mask.plan_id);
            count += 1;
        }
        assert_eq!(count, 3);
        assert_eq!(masks, 1);
    });
    drop(refusal);
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(cost.deallocations, 0);
    assert_eq!(cost.peak_bytes, 0);
    assert_eq!(cost.requested_bytes, 0);
    assert_eq!(cost.deallocated_bytes, 0);
    assert_eq!(cost.retained_bytes(), 0);
    let mask_scope =
        PreparedBodyFragment::prepare(&wake, &plan, &play, &mask.fragments[0]).unwrap();
    let identities = |scope: PreparedBodyFragment<'_>| {
        scope
            .local_fragments()
            .map(|f| f.unwrap().fragment().fragment_id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(identities(scope), identities(mask_scope));
}

#[test]
fn stale_context_and_substituted_fragment_refuse_without_changing_originals() {
    let (wake, plan, play) = fixture(None);
    let fragment = &plan.plots[0].plan.fragments[0];
    let mut stale = wake.clone();
    stale.workload_revision += 1;
    assert!(PreparedBodyFragment::prepare(&stale, &plan, &play, fragment).is_err());
    let mut wrong_play = play.clone();
    wrong_play.play_sequence += 1;
    assert!(PreparedBodyFragment::prepare(&wake, &plan, &wrong_play, fragment).is_err());
    let mut wrong_plan = plan.clone();
    wrong_plan.plan_id = "forged".into();
    assert!(PreparedBodyFragment::prepare(&wake, &wrong_plan, &play, fragment).is_err());
    for change in 0..4 {
        let mut substituted = fragment.clone();
        match change {
            0 => substituted.host_id = "another-host".into(),
            1 => substituted.boot_id = "another-boot".into(),
            2 => substituted.offer_generation.0 += 1,
            _ => substituted.fragment_id = "forged".into(),
        }
        assert!(PreparedBodyFragment::prepare(&wake, &plan, &play, &substituted).is_err());
    }
    assert!(PreparedBodyFragment::prepare(&wake, &plan, &play, fragment).is_ok());
    assert!(plan.validate_for(&wake).is_ok());
    assert!(play.validate_for(&plan));
}

#[test]
fn ambiguous_sibling_refuses_even_when_anchor_is_unambiguous() {
    let duplicate = support::plan("first", "host/local");
    let (wake, plan, play) = fixture(Some(&duplicate));
    let duplicate_fragment = &plan.plots[0].plan.fragments[0];
    assert!(PreparedBodyFragment::prepare(&wake, &plan, &play, duplicate_fragment).is_err());
    let anchor = &plan
        .plots
        .iter()
        .find(|p| p.plan.expanded_plot_id.as_str() == "second")
        .unwrap()
        .plan
        .fragments[0];
    let scope = PreparedBodyFragment::prepare(&wake, &plan, &play, anchor).unwrap();
    let (_, cost) = probe::observe(|| {
        assert_eq!(scope.local_fragments().filter(Result::is_err).count(), 2);
        assert_eq!(scope.local_fragments().filter(Result::is_ok).count(), 1);
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
}
