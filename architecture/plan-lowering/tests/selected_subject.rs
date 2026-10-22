//! Distinct authority subjects preserve exact Plan, lowering and possession checks.
extern crate alloc;
use conduit_core::*;
use conduit_plan_lowering::{lowering::lower_plan_fragment, selected_operation::*};
#[path = "../../../targets/conduitos/src/machine_membrane/selection_fixture.rs"]
mod fixture;
#[path = "common/native_possession.rs"]
mod native_possession;
use native_possession::{possession, scope};

fn kind() -> Kind {
    Kind {
        kind_id: "network/join".into(),
        kind_contract_revision: "network/join@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: "request".into(),
            value_kind: "network/request".into(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: "attachment".into(),
            value_kind: "network/attachment".into(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 256,
        },
    }
}
#[test]
fn separate_subject_requires_independent_expected_target_and_exact_current_binding() {
    let subject = KindId::from("authority/network-configurator");
    let mut scope = scope(subject.as_str());
    let kind = kind();
    let (mut fragment, _, _, placement) =
        fixture::selected(&scope, &kind, "call/join", 256, "radio", "join");
    fragment.placements[0].host_calls[0].target_kind = Some(subject.clone());
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment]).fragments.remove(0);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 3);
    scope.plan_id = fragment.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
    let (table, _handle, claim) = possession(scope);
    let definition = || SelectedOperationContract {
        kind: &kind,
        call: "call/join",
        input_bytes: 256,
        output_bytes: 256,
        resource_bytes: 1,
    };
    let selection = || SelectedOperationPlan {
        fragment: &fragment,
        lowered: &lowered,
        active: &active,
        placement_id: &placement,
    };
    assert_eq!(
        bind_selected_operation(&table, &claim, definition(), selection()),
        Err(SelectedCallRefusal::WrongBinding)
    );
    assert_eq!(
        bind_selected_operation_for_subject(&table, &claim, definition(), &subject, selection()),
        Ok(lowered.nodes[0].node)
    );
    assert_eq!(
        bind_selected_operation_for_subject(
            &table,
            &claim,
            definition(),
            &KindId::from("authority/other"),
            selection()
        ),
        Err(SelectedCallRefusal::WrongBinding)
    );
    let mut noncanonical = active.clone();
    noncanonical.play_sequence += 1;
    assert_eq!(
        bind_selected_operation_for_subject(
            &table,
            &claim,
            definition(),
            &subject,
            SelectedOperationPlan {
                active: &noncanonical,
                ..selection()
            }
        ),
        Err(SelectedCallRefusal::WrongBinding)
    );
    let mut stale = claim.clone();
    stale.boot_id = "boot/old".into();
    assert_eq!(
        bind_selected_operation_for_subject(&table, &stale, definition(), &subject, selection()),
        Err(SelectedCallRefusal::WrongBinding)
    );
    let mut wrong_claim = claim.clone();
    wrong_claim.subject_kind = kind.kind_id.clone();
    assert_eq!(
        bind_selected_operation_for_subject(
            &table,
            &wrong_claim,
            definition(),
            &subject,
            selection()
        ),
        Err(SelectedCallRefusal::WrongBinding)
    );
    let mut wrong_plan = fragment.clone();
    wrong_plan.placements[0].host_calls[0].target_kind = Some(kind.kind_id.clone());
    let identity = PlotIdentity {
        source_document_id: wrong_plan.source_document_id.clone(),
        checked_plot_id: wrong_plan.checked_plot_id.clone(),
        expanded_plot_id: wrong_plan.expanded_plot_id.clone(),
    };
    let wrong_plan = seal_plan(identity, vec![wrong_plan]).fragments.remove(0);
    let wrong_lowered = lower_plan_fragment(&wrong_plan).unwrap();
    let wrong_active = bind_active_play(
        &wrong_plan.plan_id,
        &wrong_plan.host_id,
        &wrong_plan.boot_id,
        3,
    );
    let mut wrong_scope = native_possession::scope(subject.as_str());
    wrong_scope.plan_id = wrong_plan.plan_id.clone();
    wrong_scope.active_play_id = wrong_active.active_play_id.clone();
    let (wrong_table, _wrong_handle, wrong_claim) = possession(wrong_scope);
    assert_eq!(
        bind_selected_operation_for_subject(
            &wrong_table,
            &wrong_claim,
            definition(),
            &subject,
            SelectedOperationPlan {
                fragment: &wrong_plan,
                lowered: &wrong_lowered,
                active: &wrong_active,
                placement_id: &placement
            }
        ),
        Err(SelectedCallRefusal::WrongBinding)
    );
    let mut forged = lowered.clone();
    forged.host_calls[0].target_kind = Some(kind.kind_id.clone());
    assert_eq!(
        bind_selected_operation_for_subject(
            &table,
            &claim,
            definition(),
            &subject,
            SelectedOperationPlan {
                lowered: &forged,
                ..selection()
            }
        ),
        Err(SelectedCallRefusal::WrongBinding)
    );
}
#[test]
fn ordinary_subject_keeps_the_existing_binding_contract() {
    let kind = kind();
    let mut scope = scope(kind.kind_id.as_str());
    let (fragment, lowered, active, placement) =
        fixture::selected(&scope, &kind, "call/join", 256, "radio", "join");
    scope.plan_id = fragment.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
    let (table, _handle, claim) = possession(scope);
    assert_eq!(
        bind_selected_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: &kind,
                call: "call/join",
                input_bytes: 256,
                output_bytes: 256,
                resource_bytes: 1
            },
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            }
        ),
        Ok(lowered.nodes[0].node)
    );
}
