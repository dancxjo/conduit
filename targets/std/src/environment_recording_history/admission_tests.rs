//! Independent Source-use and selected read authorization through public IMAGE init.
use super::*;

#[test]
fn selected_source_refuses_foreign_source_use_and_current_read_grants() {
    let root = Directory::new();
    let directory = root.open();
    let contract = conduit_dht11_observation::ObservationContract::prepare().unwrap();
    let bytes = json(&contract);
    publish(&directory, &bytes);
    let (checked, image, catalog) = equipment();
    for mode in 0..4 {
        let config = config();
        let mut grant = source_grant(&config);
        let mut selected = facts(bytes.len(), false);
        match mode {
            0 => grant.boot_id = "boot/foreign".into(),
            1 => grant.host_call_contract_id = "conduit.host/other@1".into(),
            2 => selected.original.grant_id = "grant/foreign".into(),
            _ => selected.boot = "boot/foreign".into(),
        }
        // SAFETY: fixture owns the directory and deliberately supplies invalid
        // independent authorization to prove it is refused, not reconstructed.
        assert!(unsafe {
            StdHost::from_selected_environment_recording_history_image(
                config,
                &image,
                &catalog,
                &checked,
                grant,
                selected.capture(&directory),
            )
        }
        .is_err());
    }
}

#[test]
fn selected_source_requires_exact_funded_pools_and_configuration_reference() {
    let root = Directory::new();
    let directory = root.open();
    let contract = conduit_dht11_observation::ObservationContract::prepare().unwrap();
    let bytes = json(&contract);
    publish(&directory, &bytes);
    let selected = facts(bytes.len(), false);
    let source = equipment_source();
    for bad in [
        source.replace(
            "source_pool: \"history/source\"",
            "source_pool: \"history/missing\"",
        ),
        source.replace(
            "memory_pool: \"history/memory\"",
            "memory_pool: \"history/missing\"",
        ),
        source.replace("slots: 67108864, bytes: 67108864", "slots: 1, bytes: 1"),
    ] {
        let result = equipment_from(&bad).and_then(|(checked, image, catalog)| {
            let config = config();
            // SAFETY: actual fixture read custody; checked pool selection is the
            // deliberate invalid input, never an invented runtime reservation.
            unsafe {
                StdHost::from_selected_environment_recording_history_image(
                    config.clone(),
                    &image,
                    &catalog,
                    &checked,
                    source_grant(&config),
                    selected.capture(&directory),
                )
            }
        });
        assert!(result.is_err());
    }
    let mut foreign = selected.reference.clone();
    foreign.lifetime.version = ResourceVersionIdentity::from_digest([99; 32]);
    let reference = foreign
        .encode()
        .unwrap()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let source = source.replace(
        "directory: \"\", reference: \"\"",
        &format!("directory: \"/independently-held/fixture\", reference: \"{reference}\""),
    );
    let (checked, image, catalog) = equipment_from(&source).unwrap();
    let config = config();
    // SAFETY: fixture retains real directory; mismatched configured reference
    // must refuse before capture rather than replacing the held selection.
    assert!(unsafe {
        StdHost::from_selected_environment_recording_history_image(
            config.clone(),
            &image,
            &catalog,
            &checked,
            source_grant(&config),
            selected.capture(&directory),
        )
    }
    .is_err());
}
