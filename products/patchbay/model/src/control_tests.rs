use super::*;
use crate::FormEditor;
use conduit_core::{AuthorityContractId, AuthorityRequirement, HostOperationContractId, KindId};
use conduit_std_host::{StdHost, ThreadTimer};

fn planned_hello() -> (FormEditor, StdHost, Plan) {
    let editor = FormEditor::from_source(
        "hello.conduit".into(),
        include_str!("../../../../forms/hello/main.conduit").into(),
    )
    .unwrap();
    let expanded = editor.expand_form("hello").unwrap();
    let host = StdHost::new();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    (editor, host, plan)
}

#[test]
fn plan_document_keeps_form_plan_and_exact_assignments_distinct() {
    let (editor, host, plan) = planned_hello();
    let document =
        PlanDocument::from_plan(PatchbayRequestId::new("plan/1").unwrap(), &plan).unwrap();
    let rendered = document.lines.join("\n");
    assert_ne!(plan.plan_id.as_str(), plan.source_document_id.as_str());
    assert!(rendered.contains(&format!("plan={}", plan.plan_id.as_str())));
    assert!(rendered.contains("GEAR operation="));
    assert!(rendered.contains("capability="));
    assert!(rendered.contains("implementation="));
    admit_run(
        &plan,
        editor.view().checked.source_document_id.as_ref().unwrap(),
        std::slice::from_ref(host.advertisement()),
    )
    .unwrap();
}

#[test]
fn stale_source_boot_realization_and_authority_are_distinct_rejections() {
    let (editor, host, plan) = planned_hello();
    let source = editor.view().checked.source_document_id.unwrap();
    let hosts = vec![host.advertisement().clone()];

    assert_eq!(
        admit_run(
            &plan,
            &conduit_core::SourceDocumentId::from("changed"),
            &hosts
        ),
        Err(ControlError::StalePlan)
    );
    let mut stale_boot = hosts.clone();
    stale_boot[0].boot_id = conduit_core::BootId::from("rebooted");
    assert_eq!(
        admit_run(&plan, &source, &stale_boot),
        Err(ControlError::StaleBoot)
    );
    let mut unavailable = hosts.clone();
    unavailable[0].capabilities.clear();
    assert_eq!(
        admit_run(&plan, &source, &unavailable),
        Err(ControlError::UnavailableRealization)
    );
    let mut denied = hosts;
    let capability = plan.fragments[0].placements[0].capability_id.clone();
    denied[0]
        .capabilities
        .iter_mut()
        .find(|offer| offer.capability_id == capability)
        .unwrap()
        .authority_requirements
        .push(AuthorityRequirement {
            contract_id: AuthorityContractId::from("authority/test"),
            host_operation_contract_id: HostOperationContractId::from("host/test"),
            subject_kind: KindId::from("subject/test"),
        });
    assert_eq!(
        admit_run(&plan, &source, &denied),
        Err(ControlError::AuthorityDenied)
    );
}

#[test]
fn completed_play_projection_keeps_exact_play_plan_and_sign() {
    let (_editor, mut host, plan) = planned_hello();
    let mut output = Vec::with_capacity(4096);
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut ThreadTimer)
        .unwrap();
    let document = PlayDocument::from_report(&plan, &report).unwrap();
    let rendered = document.lines.join("\n");
    assert_eq!(document.terminal, TerminalDisposition::Completed);
    assert!(rendered.contains(plan.plan_id.as_str()));
    assert!(rendered.contains(report.kernel.as_ref().unwrap().active_play_id.as_str()));
    assert!(rendered.contains("SIGN id="));
    assert!(rendered.contains("PRESSURE exposed=false"));
}

#[test]
fn live_link_availability_changes_do_not_mutate_sealed_line_candidates() {
    let exact = conduit_signal_conformance::triple::exact_plan().unwrap();
    let plan = exact.plan;
    let before = plan.clone();
    let document =
        PlanDocument::from_plan(PatchbayRequestId::new("plan/routes").unwrap(), &plan).unwrap();
    let mut link = exact.browser_line;
    link.availability.availability = conduit_core::LineAvailability::Unavailable;
    assert_eq!(plan, before);
    assert!(document.lines.iter().any(|line| line.contains("CANDIDATE")));
    assert_eq!(
        link.availability.availability,
        conduit_core::LineAvailability::Unavailable
    );
}
