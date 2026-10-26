//! Equivalent ordinary Body admission from rich and Core-verified representations.
use conduit_body::*;
use conduit_core::*;
use std::collections::BTreeMap;
#[path = "../../core/tests/common/sealed_state.rs"]
mod fixture;
fn core(label: &str) -> Plan {
    let mut fragment = fixture::fragment();
    fragment.source_document_id = format!("source/{label}").into();
    fragment.checked_plot_id = format!("checked/{label}").into();
    fragment.expanded_plot_id = format!("expanded/{label}").into();
    fragment.placements[0].configuration = vec![ConfigurationEntry {
        key: "program".into(),
        value: ConfigurationValue::Text("é\0immutable".repeat(64).into()),
    }];
    fixture::seal(fragment)
}
fn metadata(plan: &Plan) -> VerifiedPlanMetadata {
    assert!(verify_plan(plan));
    let mut plan = plan.clone();
    let configurations = plan
        .fragments
        .iter_mut()
        .flat_map(|f| &mut f.placements)
        .map(|g| {
            (
                g.placement_id.clone(),
                core::mem::take(&mut g.configuration),
            )
        })
        .collect::<BTreeMap<_, _>>();
    verify_plan_metadata_with_configurations(plan, |g| {
        Ok::<_, ()>(configurations[&g.placement_id].clone())
    })
    .unwrap()
}
fn fixture() -> (Wake, Vec<BodyPlotPlan>) {
    let plots: Vec<_> = ["b", "a"]
        .into_iter()
        .map(|name| {
            let plan = core(name);
            BodyPlotPlan {
                plot: ResidentPlot::new(
                    plan.source_document_id.clone(),
                    plan.checked_plot_id.clone(),
                ),
                plan,
            }
        })
        .collect();
    let workset = BodyWorkset::from_plots(plots.iter().map(|p| p.plot.clone())).unwrap();
    let body = Body::born_with_plots(workset, 1, "birth".into()).unwrap();
    (body.wake(2, "wake".into()).unwrap().1, plots)
}
fn mask() -> Plan {
    let mut fragment = core("mask").fragments.remove(0);
    let mut second = fragment.placements[0].clone();
    second.placement_id = "second".into();
    second.gear_id = "second".into();
    fragment.placements.push(second);
    fragment.startup_order.push("second".into());
    fragment.connections.push(PlannedConnection {
        connection_id: "mask/cord".into(),
        source_placement_id: "placement".into(),
        source_port_id: "current".into(),
        sink_placement_id: "second".into(),
        sink_port_id: "next".into(),
        value_kind: kind_id("fixture/byte@1"),
        resource: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 1,
        selected_line: None,
        admitted_lines: vec![],
    });
    fixture::seal(fragment)
}
fn time(basis: &str) -> BodyTimeRequirement {
    BodyTimeRequirement::new(
        basis.into(),
        BodyTimeTolerance::new(2, TemporalScale::Milliseconds),
        MonotonicDuration::new(500, TemporalScale::Milliseconds),
    )
    .unwrap()
}
fn compare(
    wake: &Wake,
    plots: Vec<BodyPlotPlan>,
    masks: Vec<BodyMaskTopology>,
    time: Option<BodyTimeRequirement>,
) -> Result<BodyPlanSeal, BodyPlanSealRefusal> {
    let tokens: Vec<_> = plots.iter().map(|p| metadata(&p.plan)).collect();
    let mask_tokens: Vec<Vec<_>> = masks
        .iter()
        .map(|m| m.chains.iter().map(|c| metadata(&c.plan)).collect())
        .collect();
    let inputs: Vec<_> = plots
        .iter()
        .zip(&tokens)
        .map(|(p, token)| VerifiedBodyPlotPlan {
            plot: p.plot.clone(),
            plan: token,
        })
        .collect();
    let mask_inputs: Vec<_> = masks
        .iter()
        .zip(&mask_tokens)
        .map(|(m, tokens)| VerifiedBodyMaskTopology {
            face: m.face.clone(),
            chains: m
                .chains
                .iter()
                .zip(tokens)
                .map(|(c, token)| VerifiedBodyMaskChain {
                    plan: token,
                    stage_placement_ids: c.stage_placement_ids.clone(),
                })
                .collect(),
        })
        .collect();
    let actual = BodyPlanSeal::capture_verified(wake, &inputs, &mask_inputs, time.clone());
    let rich = if let Some(time) = time {
        assert!(masks.is_empty());
        BodyPlan::seal_with_body_time(wake, plots, time)
    } else {
        BodyPlan::seal_with_masks(wake, plots, masks)
    };
    let expected = rich
        .map_err(BodyPlanSealRefusal::Invalid)
        .and_then(|p| BodyPlanSeal::capture(&p));
    assert_eq!(actual, expected);
    actual
}
#[test]
fn verified_capture_preserves_body_identity_mask_paths_time_and_lifecycle() {
    let (wake, plots) = fixture();
    let seal = compare(&wake, plots.clone(), vec![], None).unwrap();
    let tokens: Vec<_> = plots.iter().map(|p| metadata(&p.plan)).collect();
    let inputs: Vec<_> = plots
        .iter()
        .zip(&tokens)
        .map(|(p, token)| VerifiedBodyPlotPlan {
            plot: p.plot.clone(),
            plan: token,
        })
        .collect();
    let actual = BodyPlanSeal::capture_verified(&wake, &inputs, &[], None).unwrap();
    let expected_play = BodyPlayIdentity::bind(&seal, 7);
    assert_eq!(BodyPlayIdentity::bind(&actual, 7), expected_play);
    assert_eq!(
        wake.body_plan_ready(&actual, "ready".into())
            .unwrap()
            .body_play_started(&actual, &expected_play, "start".into()),
        wake.body_plan_ready(&seal, "ready".into())
            .unwrap()
            .body_play_started(&seal, &expected_play, "start".into())
    );
    let masks = [Some(plots[0].plot.clone()), None]
        .into_iter()
        .map(|plot| BodyMaskTopology {
            face: BodyFaceSelector {
                source_placement_id: plot.as_ref().map(|_| "placement".into()),
                plot,
            },
            chains: vec![BodyMaskChainPlan {
                plan: mask(),
                stage_placement_ids: vec!["placement".into(), "second".into()],
            }],
        })
        .collect();
    compare(&wake, plots.clone(), masks, None).unwrap();
    compare(&wake, plots, vec![], Some(time(wake.body_id.as_str()))).unwrap();
    let mut stale = wake.clone();
    stale.workload_revision += 1;
    assert_eq!(
        actual.validate_for(&stale),
        Err(BodyPlanError::StaleWorkload)
    );
    let other = Body::born_with_plots(wake.workset.clone(), 3, "other-birth".into())
        .unwrap()
        .wake(4, "other-wake".into())
        .unwrap()
        .1;
    stale = wake.clone();
    stale.wake_id = other.wake_id.clone();
    assert_eq!(actual.validate_for(&stale), Err(BodyPlanError::WrongWake));
    stale = wake.clone();
    stale.body_id = other.body_id;
    assert_eq!(actual.validate_for(&stale), Err(BodyPlanError::WrongBody));
}
#[test]
fn verified_capture_keeps_workset_and_mask_refusal_order() {
    let (wake, plots) = fixture();
    for mutation in 0..6 {
        let mut candidate = plots.clone();
        let mut masks = vec![];
        let mut requirement = None;
        match mutation {
            0 => {
                candidate.pop();
            }
            1 => candidate.push(candidate[0].clone()),
            2 => {
                candidate[0].plot = ResidentPlot::new("source/other".into(), "checked/other".into())
            }
            3 => candidate[0].plan = candidate[1].plan.clone(),
            4 => masks.push(BodyMaskTopology {
                face: BodyFaceSelector {
                    plot: None,
                    source_placement_id: None,
                },
                chains: vec![BodyMaskChainPlan {
                    plan: mask(),
                    stage_placement_ids: vec!["second".into(), "placement".into()],
                }],
            }),
            _ => requirement = Some(time("body/other")),
        }
        assert!(compare(&wake, candidate, masks, requirement).is_err());
    }
}

#[test]
fn verified_capture_preserves_the_finite_fragment_receipt_capacity() {
    let (wake, mut plots) = fixture();
    let template = plots[0].plan.fragments[0].clone();
    let fragments = (0..=MAX_BODY_SEAL_FRAGMENTS)
        .map(|index| {
            let mut fragment = template.clone();
            fragment.host_id = format!("host/{index}").into();
            fragment.placements.clear();
            fragment.states.clear();
            fragment.startup_order.clear();
            fragment
        })
        .collect();
    let original = &plots[0].plan;
    plots[0].plan = seal_plan(
        PlotIdentity {
            source_document_id: original.source_document_id.clone(),
            checked_plot_id: original.checked_plot_id.clone(),
            expanded_plot_id: original.expanded_plot_id.clone(),
        },
        fragments,
    );
    assert_eq!(
        compare(&wake, plots, vec![], None),
        Err(BodyPlanSealRefusal::Capacity)
    );
}
