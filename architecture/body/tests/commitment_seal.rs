//! Canonical identity parity, not permission for a new Boot to execute.
use conduit_body::*;
use conduit_core::*;
use sha2::{Digest, Sha256};
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../core/tests/common/sealed_state.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn text(bytes: &mut Vec<u8>, s: &str) {
    bytes.extend_from_slice(&(s.len() as u32).to_le_bytes());
    bytes.extend_from_slice(s.as_bytes());
}
fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[test]
fn borrowed_three_plot_commitments_match_legacy_body_and_play_bytes() {
    let plots: Vec<_> = (0..3)
        .map(|i| {
            ResidentPlot::new(
                format!("{i:064x}").into(),
                format!("{:064x}", i + 10).into(),
            )
        })
        .collect();
    let body = Body::born_with_plots(
        BodyWorkset::from_plots(plots.clone()).unwrap(),
        1,
        "born".into(),
    )
    .unwrap();
    let mut previous = None;
    for (sequence, boot) in [(1, "boot/one"), (2, "boot/two")] {
        let (_, wake) = body
            .wake(sequence, format!("wake/{sequence}").into())
            .unwrap();
        let full = BodyPlan::seal(
            &wake,
            plots
                .iter()
                .map(|plot| {
                    let mut fragment = fixture::fragment();
                    fragment.boot_id = boot.into();
                    for p in &mut fragment.placements {
                        p.boot_id = boot.into();
                    }
                    BodyPlotPlan {
                        plot: plot.clone(),
                        plan: seal_plan(
                            PlotIdentity {
                                source_document_id: plot.source_document_id.clone(),
                                checked_plot_id: plot.checked_plot_id.clone(),
                                expanded_plot_id: "expanded".into(),
                            },
                            vec![fragment],
                        ),
                    }
                })
                .collect(),
        )
        .unwrap();
        let pins: Vec<_> = full
            .plots
            .iter()
            .map(|p| BodyPlotSeal {
                source_document_id: p.plot.source_document_id.as_str(),
                checked_plot_id: p.plot.checked_plot_id.as_str(),
                plan_id: p.plan.plan_id.as_str(),
            })
            .collect();
        let (digest, allocation) = allocation_probe::observe(|| {
            body_plan_commitment_digest(
                full.body_id.as_str(),
                full.wake_id.as_str(),
                full.workload_revision,
                &pins,
                &[],
                None,
            )
            .unwrap()
        });
        assert_eq!(allocation.allocations, 0);
        assert_eq!(allocation.reallocations, 0);
        // Independent former Vec serializer, including domain and zero tails.
        let mut old = Vec::new();
        text(&mut old, "conduit.body/body-plan@3");
        text(&mut old, full.body_id.as_str());
        text(&mut old, full.wake_id.as_str());
        old.extend_from_slice(&full.workload_revision.to_le_bytes());
        old.extend_from_slice(&3u32.to_le_bytes());
        for p in &pins {
            text(&mut old, p.source_document_id);
            text(&mut old, p.checked_plot_id);
            text(&mut old, p.plan_id);
        }
        old.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(digest, <[u8; 32]>::from(Sha256::digest(&old)));
        assert_eq!(
            full.plan_id.as_str(),
            format!("body-plan/sha256:{}", hex(digest))
        );
        let play = BodyPlayIdentity::bind(&full, sequence);
        assert!(body_play_identity_matches(
            full.body_id.as_str(),
            full.wake_id.as_str(),
            full.plan_id.as_str(),
            sequence,
            play.active_play_id.as_str()
        ));
        assert_ne!(previous, Some(digest));
        previous = Some(digest);
        assert_eq!(
            body_plan_commitment_digest("b", "w", 0, &[], &[], None),
            Err(BodyPlanError::EmptyWorkset)
        );
        let duplicate = [pins[0], pins[0]];
        assert_eq!(
            body_plan_commitment_digest("b", "w", 0, &duplicate, &[], None),
            Err(BodyPlanError::DuplicatePlot)
        );
        let overflow = [pins[0]; MAX_BODY_PLOTS + 1];
        assert_eq!(
            body_plan_commitment_digest("b", "w", 0, &overflow, &[], None),
            Err(BodyPlanError::PlotCapacityExceeded)
        );
    }
}
