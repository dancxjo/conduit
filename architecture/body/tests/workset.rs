use conduit_body::{
    Body, BodyLifecycleError, BodyWorkset, BodyWorksetError, ResidentForm, MAX_BODY_FORMS,
};
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};

fn form(name: &str) -> ResidentForm {
    ResidentForm::new(
        SourceDocumentId::from(format!("source/{name}")),
        CheckedFormId::from(format!("checked/{name}")),
    )
}

fn body() -> Body {
    Body::born(
        SourceDocumentId::from("source/seed"),
        CheckedFormId::from("checked/seed"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
}

#[test]
fn body_retains_multiple_exact_forms_without_program_identity_or_body_replacement() {
    let born = body();
    let with_service = born
        .admit_form(form("service"), SignId::from("sign/admit-service"))
        .unwrap();
    let with_dashboard = with_service
        .admit_form(form("dashboard"), SignId::from("sign/admit-dashboard"))
        .unwrap();

    assert_eq!(with_dashboard.body_id, born.body_id);
    assert_eq!(with_dashboard.seed_id, born.seed_id);
    assert_eq!(with_dashboard.effective_workset().unwrap().len(), 3);
    assert_eq!(
        with_dashboard.admit_form(form("service"), SignId::from("sign/duplicate")),
        Err(BodyLifecycleError::DuplicateForm)
    );

    let without_seed = with_dashboard
        .remove_form(&form("seed"), SignId::from("sign/remove-seed"))
        .unwrap();
    assert_eq!(without_seed.body_id, born.body_id);
    assert_eq!(without_seed.seed_id, born.seed_id);
    assert!(!without_seed
        .effective_workset()
        .unwrap()
        .contains(&form("seed")));

    let empty = without_seed
        .remove_form(&form("dashboard"), SignId::from("sign/remove-dashboard"))
        .unwrap()
        .remove_form(&form("service"), SignId::from("sign/remove-service"))
        .unwrap();
    assert!(empty.effective_workset().unwrap().is_empty());
    assert_eq!(empty.body_id, born.body_id);
    empty.validate().unwrap();
}

#[test]
fn workset_is_canonical_bounded_by_count_and_identity_bytes() {
    let mut forward = BodyWorkset::default();
    forward.add(form("z")).unwrap();
    forward.add(form("a")).unwrap();
    let mut reverse = BodyWorkset::default();
    reverse.add(form("a")).unwrap();
    reverse.add(form("z")).unwrap();
    assert_eq!(forward, reverse);

    let mut count = BodyWorkset::default();
    for index in 0..MAX_BODY_FORMS {
        count.add(form(&format!("count-{index}"))).unwrap();
    }
    assert_eq!(
        count.add(form("overflow")),
        Err(BodyWorksetError::FormCapacityExhausted)
    );

    let mut bytes = BodyWorkset::default();
    for index in 0..15 {
        bytes
            .add(ResidentForm::new(
                SourceDocumentId::from(format!("source/{index:02}/{}", "s".repeat(85))),
                CheckedFormId::from(format!("checked/{index:02}/{}", "c".repeat(30))),
            ))
            .unwrap();
    }
    assert_eq!(
        bytes.add(ResidentForm::new(
            SourceDocumentId::from(format!("source/99/{}", "s".repeat(85))),
            CheckedFormId::from(format!("checked/99/{}", "c".repeat(30))),
        )),
        Err(BodyWorksetError::IdentityBytesExhausted)
    );
}

#[test]
fn revision_zero_evidence_migrates_to_the_seed_form_without_rebinding_body() {
    let current = body();
    let mut value = serde_json::to_value(&current).unwrap();
    value.as_object_mut().unwrap().remove("workset");
    value.as_object_mut().unwrap().remove("workload_revision");
    let legacy: Body = serde_json::from_value(value).unwrap();

    assert_eq!(legacy.body_id, current.body_id);
    assert_eq!(legacy.workload_revision, 0);
    assert_eq!(legacy.effective_workset().unwrap().forms(), &[form("seed")]);
    legacy.validate().unwrap();

    let migrated = legacy
        .admit_form(form("second"), SignId::from("sign/admit-second"))
        .unwrap();
    assert_eq!(migrated.body_id, current.body_id);
    assert_eq!(migrated.workload_revision, 2);
    assert_eq!(migrated.effective_workset().unwrap().len(), 2);
}
