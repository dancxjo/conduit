use conduit_body::{
    Body, BodyBiographyEvidence, BodyBiographyRecordKind, BodyMembership, ResidentForm,
};
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};

fn form(name: &str) -> ResidentForm {
    ResidentForm::new(
        SourceDocumentId::from(format!("source/{name}")),
        CheckedFormId::from(format!("checked/{name}")),
    )
}

#[test]
fn biography_records_exact_form_admission_and_removal_separately_from_seed_history() {
    let seed = form("seed");
    let born = Body::born(
        seed.source_document_id,
        seed.checked_form_id,
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let mut biography = BodyBiographyEvidence::born(
        born.clone(),
        BodyMembership::new(born.body_id.clone()).unwrap(),
        "Roseau".into(),
    )
    .unwrap();
    let service = form("service");
    let admitted = born
        .admit_form(service.clone(), SignId::from("sign/service-admitted"))
        .unwrap();
    let current = admitted
        .remove_form(&form("seed"), SignId::from("sign/seed-stopped"))
        .unwrap();

    biography
        .append_body_workload_events(
            current.clone(),
            &[
                (SignId::from("sign/service-admitted"), 2),
                (SignId::from("sign/seed-stopped"), 3),
            ],
        )
        .unwrap();

    assert_eq!(biography.body.body_id, born.body_id);
    assert_eq!(
        biography.body.effective_workset().unwrap().forms(),
        &[service]
    );
    assert!(matches!(
        biography.records[1].kind,
        BodyBiographyRecordKind::FormAdmitted {
            workload_revision: 1,
            ..
        }
    ));
    assert!(matches!(
        biography.records[2].kind,
        BodyBiographyRecordKind::FormRemoved {
            workload_revision: 2,
            ..
        }
    ));
    assert!(matches!(
        biography.records[0].kind,
        BodyBiographyRecordKind::Born {
            workload_revision: 0,
            ..
        }
    ));
    biography.validate().unwrap();
}
