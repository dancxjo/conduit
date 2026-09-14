use super::*;
use conduit_body::Body;
use conduit_core::SignId;

fn resident(workload: &ReviewedPeteWorkload, role: PeteWorkloadRole) -> &PeteResidentForm {
    workload
        .resident_forms
        .iter()
        .find(|resident| resident.role == role)
        .unwrap()
}

#[test]
fn pete_birth_uses_four_ordinary_non_actuating_forms_without_seed_privilege() {
    let workload = reviewed_pete_workload().unwrap();
    assert_eq!(workload.initial.len(), 4);
    assert!(!workload.initial.contains(&workload.navigation));
    assert!(workload
        .resident_forms
        .iter()
        .filter(|resident| !resident.may_request_motion)
        .all(|resident| workload.initial.contains(&resident.form)));

    let body =
        Body::born_with_forms(workload.initial.clone(), 8, SignId::from("pete/body-born")).unwrap();
    assert_eq!(body.workload_revision, 0);
    assert_eq!(body.workset, workload.initial);

    let with_navigation = workload.with_navigation().unwrap();
    assert_eq!(with_navigation.len(), 5);
    assert!(with_navigation.contains(&workload.navigation));
    let body_identity = body.body_id.clone();
    let embodied = body
        .admit_form(
            workload.navigation.clone(),
            SignId::from("pete/navigation-admitted"),
        )
        .unwrap();
    assert_eq!(embodied.body_id, body_identity);
    assert_eq!(embodied.workset, with_navigation);
    assert_eq!(embodied.workload_revision, 1);
}

#[test]
fn exact_form_requirements_and_reused_non_pete_forms_are_inspectable() {
    let workload = reviewed_pete_workload().unwrap();
    let requires = |role, kind: &str| {
        resident(&workload, role)
            .required_kinds
            .iter()
            .any(|required| required.as_str() == kind)
    };
    assert!(requires(
        PeteWorkloadRole::Situation,
        "pete/situation-select"
    ));
    assert!(requires(
        PeteWorkloadRole::AutobiographicalMemory,
        "pete/memory-retain"
    ));
    assert!(requires(
        PeteWorkloadRole::HistoricalIndex,
        "history/bounded-typed"
    ));
    assert!(requires(
        PeteWorkloadRole::Conversation,
        "house/context-to-prompt"
    ));
    assert!(requires(PeteWorkloadRole::Conversation, "llm/generate"));
    assert!(requires(
        PeteWorkloadRole::Navigation,
        "navigation/route-grid4"
    ));
    assert!(requires(
        PeteWorkloadRole::Navigation,
        "navigation/local-control"
    ));
    assert!(resident(&workload, PeteWorkloadRole::Navigation).may_request_motion);
    assert!(!resident(&workload, PeteWorkloadRole::Conversation).may_request_motion);
}

#[test]
fn authored_meaning_has_no_deployment_role_or_mechanism_facts() {
    for source in [
        PETE_SITUATION_FORM_SOURCE,
        PETE_MEMORY_FORM_SOURCE,
        BOUNDED_TYPED_HISTORY_FORM_SOURCE,
        HOUSE_CONVERSATION_FORM_SOURCE,
        BOUNDED_NAVIGATION_FORM_SOURCE,
    ] {
        for forbidden in [
            "forebrain",
            "motherbrain",
            "Brainstem",
            "WebSerial",
            "WebUSB",
            "/dev/",
            "Create opcode",
            "implementation-id",
        ] {
            assert!(
                !source.contains(forbidden),
                "authored Form contains {forbidden}"
            );
        }
    }
}
