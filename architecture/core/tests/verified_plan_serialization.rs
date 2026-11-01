//! Complete wire from opaque verified custody, with bounded transient payloads.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/sealed_state.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn full_plan(fragments: usize, placements: usize, text_bytes: usize) -> Plan {
    let mut parts = Vec::new();
    for fragment_index in 0..fragments {
        let mut fragment = fixture::fragment();
        let prototype = fragment.placements.pop().unwrap();
        fragment.states.clear();
        fragment.host_id = format!("host/{fragment_index}").into();
        fragment.boot_id = format!("boot/{fragment_index}").into();
        for index in 0..placements {
            let mut gear = prototype.clone();
            gear.host_id = fragment.host_id.clone();
            gear.boot_id = fragment.boot_id.clone();
            gear.placement_id = format!("placement/{fragment_index}/{index}").into();
            gear.gear_id = format!("gear/{fragment_index}/{index}").into();
            gear.configuration = vec![
                ConfigurationEntry {
                    key: "text".into(),
                    value: ConfigurationValue::Text(
                        format!(
                            "é\0\\\"🦀{fragment_index}/{index}{}",
                            "p".repeat(text_bytes)
                        )
                        .into(),
                    ),
                },
                ConfigurationEntry {
                    key: "flag".into(),
                    value: ConfigurationValue::Bool(index % 2 == 0),
                },
                ConfigurationEntry {
                    key: "count".into(),
                    value: ConfigurationValue::U64(u64::MAX),
                },
                ConfigurationEntry {
                    key: "signed".into(),
                    value: ConfigurationValue::I64(i64::MIN),
                },
            ];
            fragment.placements.push(gear);
        }
        fragment.startup_order = fragment
            .placements
            .iter()
            .map(|gear| gear.placement_id.clone())
            .collect();
        parts.push(fragment);
    }
    seal_plan_with_completion(
        PlotIdentity {
            source_document_id: "source".into(),
            checked_plot_id: "checked".into(),
            expanded_plot_id: "expanded".into(),
        },
        PlanCompletionPolicy::SemanticCompletion,
        parts,
    )
}
fn separate(
    full: &Plan,
) -> (
    VerifiedPlanMetadata,
    std::collections::BTreeMap<PlacementId, Vec<ConfigurationEntry>>,
) {
    let mut metadata = full.clone();
    let configurations: std::collections::BTreeMap<_, _> = metadata
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
        .map(|gear| {
            (
                gear.placement_id.clone(),
                core::mem::take(&mut gear.configuration),
            )
        })
        .collect();
    let verified = verify_plan_metadata_with_configurations(metadata, |gear| {
        Ok::<_, ()>(configurations[&gear.placement_id].clone())
    })
    .unwrap();
    (verified, configurations)
}
#[test]
fn complete_json_and_postcard_preserve_canonical_schema_and_exact_full_plan() {
    for full in [
        full_plan(1, 1, 0),
        full_plan(2, 3, 37),
        fixture::seal(fixture::fragment()),
    ] {
        assert!(verify_plan(&full));
        let (verified, configurations) = separate(&full);
        let view = verified.serialize_with_configurations(|gear| {
            Ok::<_, ()>(configurations[&gear.placement_id].clone())
        });
        let rich_json = serde_json::to_vec(&full).unwrap();
        let streamed_json = serde_json::to_vec(&view).unwrap();
        assert_eq!(streamed_json, rich_json);
        assert_eq!(
            serde_json::from_slice::<Plan>(&streamed_json).unwrap(),
            full
        );
        assert_eq!(
            postcard::to_allocvec(&view).unwrap(),
            postcard::to_allocvec(&full).unwrap()
        );
        // Repeated passes retain their verified basis but supply payloads afresh.
        assert_eq!(serde_json::to_vec(&view).unwrap(), rich_json);
        assert!(verified
            .preparation_metadata()
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .all(|gear| gear.configuration.is_empty()));
    }
}
#[test]
fn omitted_substituted_reordered_or_unavailable_configuration_refuses_both_codecs() {
    let full = full_plan(2, 3, 16);
    let (verified, configurations) = separate(&full);
    for failure in [
        "missing",
        "changed",
        "reordered",
        "unavailable",
        "last-placement",
    ] {
        let view = verified.serialize_with_configurations(|gear| {
            let mut entries = configurations[&gear.placement_id].clone();
            match failure {
                "missing" => entries.clear(),
                "changed" => entries[0].value = ConfigurationValue::Text("foreign".into()),
                "reordered" => entries.reverse(),
                "unavailable" => return Err("private supplier failure must not leak"),
                "last-placement" if gear.placement_id.as_str() == "placement/1/2" => {
                    entries.clear()
                }
                _ => (),
            }
            Ok(entries)
        });
        let json = serde_json::to_vec(&view).unwrap_err();
        assert!(!json.to_string().contains("private supplier"));
        assert!(postcard::to_allocvec(&view).is_err(), "{failure}");
        // A failed serialization never changes opaque verification evidence.
        assert_eq!(verified.plan_id(), &full.plan_id);
    }
}
#[test]
fn supplier_mutation_between_serialization_passes_cannot_reuse_previous_validation() {
    let full = full_plan(1, 1, 0);
    let (verified, configurations) = separate(&full);
    let mut calls = 0;
    let view = verified.serialize_with_configurations(|gear| {
        calls += 1;
        if calls == 1 {
            Ok::<_, ()>(configurations[&gear.placement_id].clone())
        } else {
            Ok(vec![])
        }
    });
    assert_eq!(
        serde_json::to_vec(&view).unwrap(),
        serde_json::to_vec(&full).unwrap()
    );
    assert!(serde_json::to_vec(&view).is_err());
}
#[test]
fn serialization_retires_each_placement_instead_of_retaining_all_configuration() {
    const COUNT: usize = 24;
    let full = full_plan(1, COUNT, 8192);
    let expected = serde_json::to_vec(&full).unwrap();
    let (verified, configurations) = separate(&full);
    let view = verified.serialize_with_configurations(|gear| {
        Ok::<_, ()>(configurations[&gear.placement_id].clone())
    });
    let mut output = vec![0; expected.len()];
    let ((), streaming) = allocation_probe::observe(|| {
        let mut writer = std::io::Cursor::new(output.as_mut_slice());
        serde_json::to_writer(&mut writer, &view).unwrap();
        assert_eq!(writer.position() as usize, expected.len());
    });
    assert_eq!(output, expected);
    let ((), reconstruction) = allocation_probe::observe(|| {
        let rich = full.clone();
        let mut writer = std::io::Cursor::new(output.as_mut_slice());
        serde_json::to_writer(&mut writer, &rich).unwrap();
    });
    assert_eq!(output, expected);
    assert_eq!(streaming.retained_bytes(), 0, "{streaming:?}");
    assert!(streaming.peak_bytes <= 24 * 1024, "{streaming:?}");
    assert!(
        streaming.peak_bytes * 8 < reconstruction.peak_bytes,
        "streaming={streaming:?}, rich={reconstruction:?}"
    );
    eprintln!(
        "streamed_plan_peak={} whole_plan_clone_peak={}",
        streaming.peak_bytes, reconstruction.peak_bytes
    );
}

#[test]
fn serialization_borrows_large_immutable_contract_instead_of_cloning_placement() {
    let mut fragment = fixture::fragment();
    fragment.states.clear();
    fragment.placements[0]
        .semantic_contract
        .configuration
        .push(KindConfigurationField {
            key: "immutable-default".into(),
            default_value: ConfigurationValue::Text("x".repeat(8192).into()),
            rule: KindConfigurationRule::Any,
        });
    let full = fixture::seal(fragment);
    let expected = serde_json::to_vec(&full).unwrap();
    let (verified, _) = separate(&full);
    let view = verified.serialize_with_configurations(|_| Ok::<_, ()>(Vec::new()));
    let mut output = vec![0; expected.len()];
    let ((), borrowed) = allocation_probe::observe(|| {
        let mut writer = std::io::Cursor::new(output.as_mut_slice());
        serde_json::to_writer(&mut writer, &view).unwrap();
        assert_eq!(writer.position() as usize, expected.len());
    });
    assert_eq!(output, expected);
    assert_eq!(
        borrowed.allocations + borrowed.reallocations,
        0,
        "{borrowed:?}"
    );
    // Historical per-placement reconstruction, measured independently of output
    // backing and configuration supply; it duplicates this immutable contract.
    let (_, historical) = allocation_probe::observe(|| {
        let complete = verified.preparation_metadata().fragments[0].placements[0].clone();
        serde_json::to_writer(std::io::sink(), &complete).unwrap();
    });
    assert!(historical.peak_bytes >= 8192, "{historical:?}");
    assert_eq!(historical.retained_bytes(), 0);
    eprintln!(
        "borrowed_placement_peak={} historical_placement_clone_peak={} historical_allocations={}",
        borrowed.peak_bytes, historical.peak_bytes, historical.allocations
    );
}

#[test]
fn canonical_configuration_replacement_preserves_optional_placement_fields() {
    for present in [false, true] {
        let mut fragment = fixture::fragment();
        fragment.states.clear();
        let gear = &mut fragment.placements[0];
        if present {
            gear.source_span = Some(SourceSpan {
                start: 1,
                end: 9,
                line: 1,
                column: 2,
                end_line: 1,
                end_column: 10,
            });
            gear.base = Some(BaseProviderBinding {
                base_id: "base".into(),
                provider_instance_id: "provider".into(),
                provider_generation: 7,
                implementation_id: "base/implementation".into(),
                mechanism_family: "base/family".into(),
                enforcement_class: BaseEnforcementClass::HardwareGated,
            });
            gear.terminal_transductions
                .push(TerminalTransductionProfile {
                    input_port_id: "next".into(),
                    output_port_id: "current".into(),
                    normal_close: NormalCloseTransduction::NotAccepted,
                    abnormal: AbnormalTerminalTransduction::NotAccepted,
                    cancellation: CancellationTransduction::NotCancellable,
                });
            gear.host_calls.push(HostCallRequirement {
                contract_id: "call".into(),
                target_kind: Some(gear.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: 64,
                maximum_output_bytes: 64,
            });
        }
        let full = fixture::seal(fragment);
        let (verified, configurations) = separate(&full);
        let view = verified.serialize_with_configurations(|gear| {
            Ok::<_, ()>(configurations[&gear.placement_id].clone())
        });
        assert_eq!(
            serde_json::to_vec(&view).unwrap(),
            serde_json::to_vec(&full).unwrap()
        );
        assert_eq!(
            postcard::to_allocvec(&view).unwrap(),
            postcard::to_allocvec(&full).unwrap()
        );
        let json = serde_json::to_value(&view).unwrap();
        let gear = &json["fragments"][0]["placements"][0];
        assert_eq!(gear.get("source_span").is_some(), present);
        assert_eq!(gear.get("base").is_some(), present);
        assert_eq!(gear.get("terminal_transductions").is_some(), present);
        assert!(gear["configuration"].as_array().unwrap().is_empty());
    }
}
