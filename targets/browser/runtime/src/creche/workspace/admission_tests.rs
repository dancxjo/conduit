use super::*;

#[test]
fn catalogue_and_dynamic_planning_evidence_invalidate_retained_admission() {
    let mut observed = crate::browser_pointer::advertisement();
    let host = observed.host_id.clone();
    let boot = observed.boot_id.clone();
    let source = "unit smoot : Distance = { reference: m, scale: 1.7018 }";
    let key = admission_key(source, &[observed.clone()], &host, &boot, &[]).unwrap();
    assert_eq!(
        key,
        admission_key(source, &[observed.clone()], &host, &boot, &[]).unwrap()
    );
    assert_ne!(
        key,
        admission_key(
            "unit smoot : Distance = { reference: m, scale: 2 }",
            &[observed.clone()],
            &host,
            &boot,
            &[]
        )
        .unwrap()
    );
    observed.capabilities[0].limits.max_queue_bytes += 1;
    assert_ne!(
        key,
        admission_key(source, &[observed.clone()], &host, &boot, &[]).unwrap()
    );
    observed.boot_id = BootId::from("replacement-boot");
    assert_ne!(
        key,
        admission_key(source, &[observed], &host, &boot, &[]).unwrap()
    );
    let line = JoinedLineObservation {
        host_id: host.clone(),
        boot_id: boot.clone(),
        carrier: "websocket/new-line".into(),
    };
    assert_ne!(
        admission_key(source, &[], &host, &boot, &[]).unwrap(),
        admission_key(source, &[], &host, &boot, &[line]).unwrap()
    );
}

#[test]
fn malformed_catalogue_never_reuses_previous_admission() {
    LIBRARY_ADMISSION.with(|cache| {
        *cache.borrow_mut() = Some(LibraryAdmission {
            key: [7; 32],
            available: vec![true],
        })
    });
    assert!(
        workspace_library("{}", &[], &HostId::from("host"), &BootId::from("boot"), &[]).is_err()
    );
    LIBRARY_ADMISSION.with(|cache| assert_eq!(cache.borrow().as_ref().unwrap().key, [7; 32]));
}

#[test]
fn theremin_shelf_admission_derives_exact_dynamic_backs_from_its_source() {
    let source = include_str!("../../../../../../plots/pocket-theremin/main.conduit");
    let bundle = serde_json::json!({"schema":"conduit.creche/reviewed-plot-bundle@2",
        "plots":[{"slug":"pocket-theremin","entry":"phone-two-axis-controller","face_profile":1,"source":source}]}).to_string();
    let inventory = super::super::initial_plots::reviewed_inventory(&bundle).unwrap();
    let plot = &inventory.plots[0];
    let entry = CatalogPlot {
        slug: "pocket-theremin".into(),
        title: "Pocket Theremin".into(),
        entry: plot.name.clone(),
        source: source.into(),
        source_document_id: plot.source_document_id.clone(),
        checked_plot_id: plot.checked_plot_id.clone(),
        face_profile: 1,
        required_kinds: plot.required_kinds.clone(),
        unavailable_hint: String::new(),
        graceful_fallback: None,
    };
    let host = HostId::from("browser/theremin-shelf");
    let boot = BootId::from("boot/theremin-shelf");
    let observed = crate::installed_browser::advertisement(host.clone(), boot.clone());
    catalog_plot_plan(
        &entry,
        &[observed],
        &host,
        &boot,
        &[],
        &mut super::super::catalog_preparation::CatalogPreparation::default(),
    )
    .unwrap();
}
