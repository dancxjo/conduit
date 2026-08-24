use crate::{browser_product, form_source};
use conduit_core::ConnectionBase;
use std::path::PathBuf;

fn form() -> conduit_form::ExpandedCanonicalForm {
    form_source::load_signal(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/signal-demo.conduit"),
    )
    .unwrap()
    .expand_entry()
    .unwrap()
}

fn plan(
    instance: u64,
) -> (
    crate::product_execution::ProductExecutionContext,
    conduit_core::Plan,
) {
    let context = browser_product::context_for_instance(instance).unwrap();
    let (source, sink) = browser_product::advertisements_for(instance);
    let placements = conduit_planner::PlacementChoices {
        by_gear: std::collections::BTreeMap::from([
            (
                conduit_core::GearId::from("signal-demo/pulse"),
                conduit_planner::PlacementChoice {
                    host_id: source.host_id,
                    capability_id: "pulse-1".into(),
                },
            ),
            (
                conduit_core::GearId::from("signal-demo/show"),
                conduit_planner::PlacementChoice {
                    host_id: sink.host_id,
                    capability_id: "dom-show-1".into(),
                },
            ),
        ]),
    };
    let plan = context.plan_with_placements(&form(), &placements).unwrap();
    (context, plan)
}

#[test]
fn launched_browser_identity_is_exact_plan_truth_and_replacement_stales_old_plan() {
    let (_, old) = plan(41);
    let (replacement, new) = plan(42);
    assert_ne!(old.plan_id, new.plan_id);
    assert_ne!(old.fragments[1].boot_id, new.fragments[1].boot_id);
    let error = replacement.validate_plan(&old).unwrap_err();
    assert!(error.contains("has no runtime handle") || error.contains("stale Boot/offer identity"));
}

#[test]
fn wrong_browser_boot_line_and_absent_line_refuse_before_play() {
    let (source, sink) = browser_product::advertisements_for(7);
    let mut stale = conduit_signal::distributed_websocket_line_offer_for_endpoints(
        source.host_id.clone(),
        source.boot_id.clone(),
        sink.host_id.clone(),
        sink.boot_id.clone(),
    );
    stale.binding.sink.boot_id = "product/browser/stale/boot".into();
    let context = crate::product_execution::ProductExecutionContext::new(
        vec![source.clone(), sink.clone()],
        vec![
            crate::product_execution::ProductRuntime::coordinated(source),
            crate::product_execution::ProductRuntime::coordinated(sink),
        ],
        vec![ConnectionBase::WebSocket],
        vec![stale],
    )
    .unwrap();
    let (_, ordinary) = plan(7);
    let placements = ordinary
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| {
            (
                placement.gear_id.clone(),
                conduit_planner::PlacementChoice {
                    host_id: placement.host_id.clone(),
                    capability_id: placement.capability_id.clone(),
                },
            )
        })
        .collect();
    assert!(context
        .plan_with_placements(
            &form(),
            &conduit_planner::PlacementChoices {
                by_gear: placements
            }
        )
        .is_err());
}

#[test]
fn unchanged_signal_form_still_runs_one_host_std_without_browser_admission() {
    let mut context = crate::product_execution::ProductExecutionContext::local_std().unwrap();
    let plan = context.plan(&form(), None).unwrap();
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(
        (
            plan.fragments[0].placements.len(),
            plan.fragments[0].connections.len()
        ),
        (2, 1),
        "unexpected local Signal fragment: {:?}",
        plan.fragments[0]
    );
    assert!(plan.fragments[0]
        .connections
        .iter()
        .all(|connection| connection.selected_line.is_none()));
    let execution = context.execute(plan, &mut Vec::new()).unwrap();
    assert!(execution.observations.iter().any(|observation| matches!(
        observation.kind,
        conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Completed
        }
    )));
}
