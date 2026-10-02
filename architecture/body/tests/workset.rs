use conduit_body::{
    Body, BodyLifecycleError, BodyWorkset, BodyWorksetError, ResidentPlot, MAX_BODY_PLOTS,
};
use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};

fn plot(name: &str) -> ResidentPlot {
    ResidentPlot::new(
        SourceDocumentId::from(format!("source/{name}")),
        CheckedPlotId::from(format!("checked/{name}")),
    )
}

fn body() -> Body {
    Body::born(
        SourceDocumentId::from("source/seed"),
        CheckedPlotId::from("checked/seed"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
}

#[test]
fn body_retains_multiple_exact_plots_without_program_identity_or_body_replacement() {
    let born = body();
    let with_service = born
        .admit_plot(plot("service"), SignId::from("sign/admit-service"))
        .unwrap();
    let with_dashboard = with_service
        .admit_plot(plot("dashboard"), SignId::from("sign/admit-dashboard"))
        .unwrap();

    assert_eq!(with_dashboard.body_id, born.body_id);
    assert_eq!(with_dashboard.effective_workset().unwrap().len(), 3);
    assert_eq!(
        with_dashboard.admit_plot(plot("service"), SignId::from("sign/duplicate")),
        Err(BodyLifecycleError::DuplicatePlot)
    );

    let without_seed = with_dashboard
        .remove_plot(&plot("seed"), SignId::from("sign/remove-seed"))
        .unwrap();
    assert_eq!(without_seed.body_id, born.body_id);
    assert!(!without_seed
        .effective_workset()
        .unwrap()
        .contains(&plot("seed")));

    let empty = without_seed
        .remove_plot(&plot("dashboard"), SignId::from("sign/remove-dashboard"))
        .unwrap()
        .remove_plot(&plot("service"), SignId::from("sign/remove-service"))
        .unwrap();
    assert!(empty.effective_workset().unwrap().is_empty());
    assert_eq!(empty.body_id, born.body_id);
    empty.validate().unwrap();
}

#[test]
fn workset_is_canonical_bounded_by_count_and_identity_bytes() {
    let mut forward = BodyWorkset::default();
    forward.add(plot("z")).unwrap();
    forward.add(plot("a")).unwrap();
    let mut reverse = BodyWorkset::default();
    reverse.add(plot("a")).unwrap();
    reverse.add(plot("z")).unwrap();
    assert_eq!(forward, reverse);

    let mut count = BodyWorkset::default();
    for index in 0..MAX_BODY_PLOTS {
        count.add(plot(&format!("count-{index}"))).unwrap();
    }
    assert_eq!(
        count.add(plot("overflow")),
        Err(BodyWorksetError::PlotCapacityExhausted)
    );

    let mut bytes = BodyWorkset::default();
    for index in 0..15 {
        bytes
            .add(ResidentPlot::new(
                SourceDocumentId::from(format!("source/{index:02}/{}", "s".repeat(85))),
                CheckedPlotId::from(format!("checked/{index:02}/{}", "c".repeat(30))),
            ))
            .unwrap();
    }
    assert_eq!(
        bytes.add(ResidentPlot::new(
            SourceDocumentId::from(format!("source/99/{}", "s".repeat(85))),
            CheckedPlotId::from(format!("checked/99/{}", "c".repeat(30))),
        )),
        Err(BodyWorksetError::IdentityBytesExhausted)
    );
}

#[test]
fn revision_zero_is_the_initial_workload_revision() {
    let current = body();
    assert_eq!(current.workload_revision, 0);
    assert_eq!(
        current.effective_workset().unwrap().plots(),
        &[plot("seed")]
    );
    let migrated = current
        .admit_plot(plot("second"), SignId::from("sign/admit-second"))
        .unwrap();
    assert_eq!(migrated.workload_revision, 1);
    assert_eq!(migrated.effective_workset().unwrap().len(), 2);
}

#[test]
fn birth_accepts_zero_one_or_many_initial_plots_without_privileging_order() {
    let empty =
        Body::born_with_plots(BodyWorkset::default(), 8, SignId::from("sign/empty-born")).unwrap();
    assert!(empty.workset.is_empty());

    let forward = Body::born_with_plots(
        BodyWorkset::from_plots([plot("clock"), plot("lantern")]).unwrap(),
        9,
        SignId::from("sign/many-born"),
    )
    .unwrap();
    let reverse = Body::born_with_plots(
        BodyWorkset::from_plots([plot("lantern"), plot("clock")]).unwrap(),
        9,
        SignId::from("sign/many-born"),
    )
    .unwrap();
    assert_eq!(forward.body_id, reverse.body_id);
    assert_eq!(forward.workset.len(), 2);
    assert_eq!(forward.workload_revision, 0);
    assert!(matches!(
        forward.events.as_slice(),
        [conduit_body::BodyLifecycleEvent::Born {
            initial_workset,
            workload_revision: 0,
            ..
        }] if initial_workset == &forward.workset
    ));
}

#[test]
fn historical_seed_biography_decodes_only_as_explicit_v1_evidence() {
    let current = body();
    let legacy = serde_json::json!({
        "schema": "conduit.body/biography-evidence@1",
        "body_id": current.body_id,
        "friendly_name": "Historical Roseau",
        "initial_program": "Morse Network",
        "body": {
            "body_id": current.body_id,
            "seed_id": "legacy-seed-id",
            "source_document_id": "source/seed",
            "checked_plot_id": "checked/seed",
            "birth_sequence": 1,
            "state": "Lulled",
            "sign_ids": ["sign/born"],
            "events": [{"Born": {"sign_id": "sign/born"}}]
        }
    });
    let decoded: conduit_body::HistoricalSeedBiographyV1 = serde_json::from_value(legacy).unwrap();
    assert!(decoded.validate_historical());
    assert!(decoded.disclosure_label().contains("Legacy Seed"));
    assert!(serde_json::from_value::<Body>(serde_json::to_value(&decoded.body).unwrap()).is_err());
}
