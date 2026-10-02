use conduit_body::{
    Body, BodyBiographyEvidence, BodyBiographyRecordKind, BodyMembership, ResidentPlot,
};
use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};

fn plot(name: &str) -> ResidentPlot {
    ResidentPlot::new(
        SourceDocumentId::from(format!("source/{name}")),
        CheckedPlotId::from(format!("checked/{name}")),
    )
}

#[test]
fn biography_records_exact_plot_admission_and_removal_separately_from_seed_history() {
    let seed = plot("seed");
    let born = Body::born(
        seed.source_document_id,
        seed.checked_plot_id,
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
    let service = plot("service");
    let admitted = born
        .admit_plot(service.clone(), SignId::from("sign/service-admitted"))
        .unwrap();
    let current = admitted
        .remove_plot(&plot("seed"), SignId::from("sign/seed-stopped"))
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
        biography.body.effective_workset().unwrap().plots(),
        &[service]
    );
    assert!(matches!(
        biography.records[1].kind,
        BodyBiographyRecordKind::PlotAdmitted {
            workload_revision: 1,
            ..
        }
    ));
    assert!(matches!(
        biography.records[2].kind,
        BodyBiographyRecordKind::PlotRemoved {
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
