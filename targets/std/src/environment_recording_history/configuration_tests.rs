use super::*;
use conduit_core::*;
use conduit_host_make::{check_host_configuration, parse_host_configuration_conduit};
use serde_json::{json, Value};

fn reference() -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: ResourceExtent {
            bytes: 2048,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn fields() -> serde_json::Map<String, Value> {
    json!({
        "source_pool": "recording/source", "memory_pool": "recording/memory",
        "capture_access": "operator-admitted", "directory": "/nonexistent/recording",
        "reference": hex(&reference().encode().unwrap()),
        "producer_host": "reported/host", "producer_boot": "reported/boot",
        "producer_plan": "reported/plan", "producer_play": "reported/play",
        "temperature_source": "reported/temperature-source",
        "temperature_channel": "reported/temperature-channel", "temperature_name": "Temperature",
        "humidity_source": "reported/humidity-source",
        "humidity_channel": "reported/humidity-channel", "humidity_name": "Relative humidity",
        "tail": 16
    })
    .as_object()
    .unwrap()
    .clone()
}
fn checked(
    values: &serde_json::Map<String, Value>,
    implementation: &str,
) -> Result<CheckedHostConfiguration, String> {
    let initialization = values
        .iter()
        .map(|(key, value)| format!("{key}: {value}"))
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!(
        r#"host recording-selector-fixture {{
 schema = 1
 target = {{architecture: "x86_64", machine: "computer", os: "linux"}}
 base = {{kind: "{ENVIRONMENT_RECORDING_HISTORY_BASE}", implementation: "{implementation}", initialization: {{{initialization}}}}}
 capability = {{kind: "data/environment-recording-history", pinned_implementation: "{implementation}"}}
 need = {{id: "recording/source", class: "conduit.resource/environment-recording-history@1", slots: 1, bytes: 16384}}
 need = {{id: "recording/memory", class: "conduit.resource/runtime-memory@1", slots: 67108864, bytes: 67108864}}
 limits = {{static_memory_bytes: 16777216, heap_arena_bytes: 268435456, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}}
}}"#
    );
    let parsed = parse_host_configuration_conduit(&text).map_err(|why| format!("{why:?}"))?;
    static MAKE: std::sync::OnceLock<(
        conduit_host_make::MakeCatalog,
        conduit_host_make::MakePackageSet,
    )> = std::sync::OnceLock::new();
    let (catalog, packages) = MAKE.get_or_init(|| {
        let packages = conduit_workspace_make::package_set();
        let catalog = conduit_host_make::MakeCatalog::canonical().with_packages(&packages);
        (catalog, packages)
    });
    check_host_configuration(parsed, catalog, packages).map_err(|why| format!("{why:?}"))
}
fn refuses(values: &serde_json::Map<String, Value>) {
    // Actual Make schema or this checked parser must reject invalid input.
    assert!(
        checked(values, ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION)
            .and_then(
                |config| CheckedEnvironmentRecordingHistory::from_configuration(&config)
                    .map(|_| ())
            )
            .is_err()
    );
}
#[test]
fn actual_make_sixteen_fields_preserve_exact_selection_without_io() {
    let values = fields();
    assert_eq!(values.len(), 16);
    let config = checked(&values, ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION).unwrap();
    let selected = CheckedEnvironmentRecordingHistory::from_configuration(&config).unwrap();
    assert_eq!(selected.source_pool, "recording/source");
    assert_eq!(selected.memory_pool, "recording/memory");
    assert_eq!(selected.directory, Some("/nonexistent/recording"));
    assert_eq!(selected.reference, Some(reference()));
    assert_eq!(
        selected.producer,
        [
            "reported/host",
            "reported/boot",
            "reported/plan",
            "reported/play"
        ]
    );
    assert_eq!(
        selected.channels[0],
        [
            "reported/temperature-source",
            "Temperature",
            "reported/temperature-channel",
            "Temperature"
        ]
    );
    assert_eq!(
        selected.channels[1],
        [
            "reported/humidity-source",
            "Relative humidity",
            "reported/humidity-channel",
            "Relative humidity"
        ]
    );
    assert_eq!(selected.tail, 16);
    let mut external = values;
    external.insert("directory".into(), json!(""));
    external.insert("reference".into(), json!(""));
    let config = checked(&external, ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION).unwrap();
    let selected = CheckedEnvironmentRecordingHistory::from_configuration(&config).unwrap();
    assert!(selected.directory.is_none() && selected.reference.is_none());
}
#[test]
fn actual_make_refuses_implementation_context_tail_pools_and_field_drift() {
    assert!(checked(&fields(), "std/uninstalled-recording-worker@1").is_err());
    for (key, value) in [
        ("producer_host", json!("")),
        ("producer_boot", json!("b".repeat(129))),
        ("producer_plan", json!("reported\nplan")),
        ("producer_play", json!(4)),
        ("tail", json!(0)),
        ("tail", json!(17)),
        ("tail", json!("16")),
        ("memory_pool", json!("recording/source")),
        ("source_pool", json!("")),
        ("capture_access", json!("not-admitted")),
        ("humidity_source", json!("reported/temperature-source")),
        ("temperature_channel", json!("reported/temperature-source")),
    ] {
        let mut values = fields();
        values.insert(key.into(), value);
        refuses(&values);
    }
    let mut missing = fields();
    missing.remove("producer_boot");
    refuses(&missing);
    let mut extra = fields();
    extra.insert("unknown_selector".into(), json!("ignored"));
    refuses(&extra);
}
#[test]
fn selected_reference_requires_canonical_exact_immutable_json_profile() {
    let encoded = hex(&reference().encode().unwrap());
    for (directory, selector) in [
        ("", encoded.as_str()),
        ("/selected", ""),
        ("relative", encoded.as_str()),
        ("/selected", "zz"),
        ("/selected", "0"),
    ] {
        let mut values = fields();
        values.insert("directory".into(), json!(directory));
        values.insert("reference".into(), json!(selector));
        refuses(&values);
    }
    for selector in [format!("{encoded}00"), encoded[..encoded.len() - 2].into()] {
        let mut values = fields();
        values.insert("reference".into(), json!(selector));
        refuses(&values);
    }
    let original = reference();
    let mut invalid = Vec::new();
    let mut wrong = original.clone();
    wrong.content_profile = "value/text".into();
    invalid.push(wrong);
    let mut wrong = original.clone();
    wrong.access_class = "resource/other".into();
    invalid.push(wrong);
    let mut wrong = original.clone();
    wrong.extent.bytes = 4097;
    invalid.push(wrong);
    let mut wrong = original.clone();
    wrong.extent.items = None;
    invalid.push(wrong);
    let mut wrong = original;
    wrong.lifetime.expires_at = Some(TemporalInstant {
        ticks: 1,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/expiry".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    });
    invalid.push(wrong);
    for wrong in invalid {
        let mut values = fields();
        values.insert("reference".into(), json!(hex(&wrong.encode().unwrap())));
        refuses(&values);
    }
}

#[test]
fn template_defaults_do_not_supply_omitted_checked_initialization_fields() {
    for key in ["tail", "directory", "reference"] {
        let mut values = fields();
        values.remove(key);
        let error = checked(&values, ENVIRONMENT_RECORDING_HISTORY_IMPLEMENTATION).unwrap_err();
        assert!(error.contains(&format!("missing initialization field '{key}'")));
    }
}
