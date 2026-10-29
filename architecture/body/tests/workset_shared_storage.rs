//! Immutable storage only: sharing conveys no verification or execution grant.
use conduit_body::{
    Body, BodyBiographyEvidence, BodyBiographyRecordKind, BodyLifecycleEvent, BodyMembership,
    BodyPlan, BodyPlotPlan, BodyWorkset, BodyWorksetError, ResidentPlot,
};
use conduit_core::{seal_plan, ExpandedPlotId, PlotIdentity};
use serde::Serialize;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn plot(byte: char) -> ResidentPlot {
    ResidentPlot::new(byte.to_string().repeat(64).into(), "f".repeat(64).into())
}
fn workset() -> BodyWorkset {
    BodyWorkset::from_plots([plot('a'), plot('b')]).unwrap()
}
#[derive(Serialize)]
struct HistoricalWire<'a> {
    plots: &'a [ResidentPlot],
}

#[test]
fn exact_workset_clones_share_without_allocating_and_retire_with_last_owner() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BodyWorkset>();
    const COPIES: usize = 8;
    let original = workset();
    let mut shared = Vec::with_capacity(COPIES);
    let (_, measured) = allocation_probe::observe(|| {
        for _ in 0..COPIES {
            shared.push(original.clone());
        }
    });
    assert_eq!(measured.allocations, 0);
    assert_eq!(measured.reallocations, 0);
    for copy in &shared {
        assert!(core::ptr::eq(original.plots(), copy.plots()));
    }
    // Exactly the former private Vec clone, with the same complete ID bytes.
    let old = original.plots().to_vec();
    let mut historical = Vec::with_capacity(COPIES);
    let (_, legacy) = allocation_probe::observe(|| {
        for _ in 0..COPIES {
            historical.push(old.clone());
        }
    });
    // Every measured clone remains live; no payload is freed in this scope.
    assert_eq!(legacy.deallocations, 0);
    assert!(legacy.requested_bytes >= 1024);
    assert_eq!(legacy.allocations, COPIES * 5);
    println!(
        "workset copies={COPIES} old_retained={} new_retained={} allocation_delta={}",
        legacy.requested_bytes, measured.requested_bytes, legacy.allocations
    );
    let ((), dropped) = allocation_probe::observe(|| drop(original));
    assert_eq!(
        dropped.deallocations, 0,
        "other exact owners still retain the bytes"
    );
    assert_eq!(shared[0].len(), 2);
    let last = shared.pop().unwrap();
    drop(shared);
    let ((), released) = allocation_probe::observe(|| drop(last));
    assert!(
        released.deallocations >= 5,
        "last owner releases canonical ID storage"
    );
}

#[test]
fn successful_cow_preserves_body_wakes_and_checked_plan_worksets() {
    let source = workset();
    let body = Body::born_with_plots(source.clone(), 1, "born".into()).unwrap();
    let (_, wake) = body.wake(1, "wake".into()).unwrap();
    let plans = source
        .plots()
        .iter()
        .map(|plot| BodyPlotPlan {
            plot: plot.clone(),
            plan: seal_plan(
                PlotIdentity {
                    source_document_id: plot.source_document_id.clone(),
                    checked_plot_id: plot.checked_plot_id.clone(),
                    expanded_plot_id: ExpandedPlotId::from("e".repeat(64)),
                },
                vec![],
            ),
        })
        .collect();
    let plan = BodyPlan::seal(&wake, plans).unwrap();
    let retained_plan = plan;
    let mut next = source.clone();
    next.add(plot('c')).unwrap();
    next.remove(&plot('a')).unwrap();
    for previous in [
        &source,
        &body.workset,
        &wake.workset,
        &retained_plan.workset,
    ] {
        assert_eq!(previous.len(), 2);
        assert!(previous.contains(&plot('a')));
        assert!(!previous.contains(&plot('c')));
        assert!(core::ptr::eq(source.plots(), previous.plots()));
    }
    assert!(next.contains(&plot('c')));
    assert!(!next.contains(&plot('a')));
}

#[test]
fn refused_mutations_do_not_copy_and_wire_remains_exact() {
    let original = workset();
    let mut current = original.clone();
    let duplicate = plot('a');
    let (result, allocation) = allocation_probe::observe(|| current.add(duplicate));
    assert_eq!(result, Err(BodyWorksetError::DuplicatePlot));
    assert_eq!(allocation.allocations, 0);
    let absent = plot('c');
    let (result, allocation) = allocation_probe::observe(|| current.remove(&absent));
    assert_eq!(result, Err(BodyWorksetError::PlotAbsent));
    assert_eq!(allocation.allocations, 0);
    assert!(core::ptr::eq(original.plots(), current.plots()));
    for value in [&original, &BodyWorkset::default()] {
        let old = HistoricalWire {
            plots: value.plots(),
        };
        assert_eq!(
            serde_json::to_vec(value).unwrap(),
            serde_json::to_vec(&old).unwrap()
        );
        assert_eq!(
            postcard::to_allocvec(value).unwrap(),
            postcard::to_allocvec(&old).unwrap()
        );
        let roundtrip: BodyWorkset =
            serde_json::from_slice(&serde_json::to_vec(value).unwrap()).unwrap();
        assert_eq!(&roundtrip, value);
    }
    let malformed = HistoricalWire {
        plots: &[plot('b'), plot('a')],
    };
    let decoded: BodyWorkset =
        serde_json::from_slice(&serde_json::to_vec(&malformed).unwrap()).unwrap();
    assert_eq!(
        decoded.validate(),
        Err(BodyWorksetError::DuplicatePlot),
        "decode does not sort or validate on behalf of its caller"
    );
}

#[test]
fn empty_and_refused_empty_mutations_allocate_nothing() {
    let (mut empty, allocation) = allocation_probe::observe(BodyWorkset::default);
    assert_eq!(allocation.allocations, 0);
    let invalid = ResidentPlot::new("".into(), "".into());
    let (result, allocation) = allocation_probe::observe(|| empty.add(invalid));
    assert_eq!(result, Err(BodyWorksetError::InvalidPlotIdentity));
    assert_eq!(allocation.allocations, 0);
    empty.add(plot('a')).unwrap();
    empty.remove(&plot('a')).unwrap();
    let (cloned, allocation) = allocation_probe::observe(|| empty.clone());
    assert_eq!(allocation.allocations, 0);
    assert_eq!(cloned, BodyWorkset::default());
}

#[test]
fn actual_birth_history_wake_and_plan_share_but_decode_is_not_an_interner() {
    for decoded in [false, true] {
        let body = Body::born_with_plots(workset(), 1, "born".into()).unwrap();
        let membership = BodyMembership::new(body.body_id.clone()).unwrap();
        let evidence = BodyBiographyEvidence::born(body, membership, "instrument".into()).unwrap();
        let evidence: BodyBiographyEvidence = if decoded {
            serde_json::from_slice(&serde_json::to_vec(&evidence).unwrap()).unwrap()
        } else {
            evidence
        };
        evidence.validate().unwrap();
        let (_, wake) = evidence.body.wake(1, "wake".into()).unwrap();
        let plans = wake
            .workset
            .plots()
            .iter()
            .map(|plot| BodyPlotPlan {
                plot: plot.clone(),
                plan: seal_plan(
                    PlotIdentity {
                        source_document_id: plot.source_document_id.clone(),
                        checked_plot_id: plot.checked_plot_id.clone(),
                        expanded_plot_id: ExpandedPlotId::from("e".repeat(64)),
                    },
                    vec![],
                ),
            })
            .collect();
        let plan = BodyPlan::seal(&wake, plans).unwrap();
        let retained_plan = plan;
        let born = match &evidence.body.events[0] {
            BodyLifecycleEvent::Born {
                initial_workset, ..
            } => initial_workset,
            _ => panic!("birth fixture"),
        };
        let recorded = match &evidence.records[0].kind {
            BodyBiographyRecordKind::Born {
                initial_workset, ..
            } => initial_workset,
            _ => panic!("birth record fixture"),
        };
        let retained = [
            &evidence.body.workset,
            born,
            recorded,
            &wake.workset,
            &retained_plan.workset,
        ];
        let mut copies = Vec::with_capacity(retained.len());
        let (_, avoided) = allocation_probe::observe(|| {
            for (at, value) in retained.iter().enumerate() {
                if retained[..at]
                    .iter()
                    .any(|old| core::ptr::eq(old.plots(), value.plots()))
                {
                    copies.push(value.plots().to_vec());
                }
            }
        });
        assert_eq!(copies.len(), if decoded { 2 } else { 4 });
        assert_eq!(avoided.deallocations, 0);
        if !decoded {
            assert!(avoided.requested_bytes > 1024);
        }
        println!("actual workset owners={} decoded={decoded} avoided_deep_copies={} avoided_owned_heap={}",
            retained.len(), copies.len(), avoided.requested_bytes);
    }
}
