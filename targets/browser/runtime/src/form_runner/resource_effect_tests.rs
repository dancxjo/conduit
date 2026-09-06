//! Real planning and kernel requests; external storage acknowledgements are explicit fixtures.
use super::*;
use crate::installed_browser::test_json;
use conduit_core::*;
use conduit_form::{KindDefinition, KindSignature};
use std::collections::BTreeMap;

fn prepare(
    publish: bool,
    content: &[u8],
    grant: bool,
) -> Result<(TourScheduler, PlanFragment), String> {
    let boot = if publish { "boot/write" } else { "boot/read" };
    let (template, reference) =
        crate::resource_snapshot::tests::placement(boot, publish, content.len());
    let binding = &template.resources[0];
    let mut host = crate::snapshot_advertisement(
        template.host_id.clone(),
        template.boot_id.clone(),
        ResourceOffer {
            pool_id: binding.pool_id.clone(),
            class_id: binding.class_id.clone(),
            capacity_units: 1,
            compute: None,
            content: binding.content.clone(),
        },
    )?;
    let offer = host.capabilities.last().unwrap();
    let authority = AuthorityGrant {
        grant_id: "snapshot-proof-grant".into(),
        contract_id: crate::resource_snapshot::AUTHORITY_CONTRACT.into(),
        host_operation_contract_id: offer.host_operations[0].contract_id.clone(),
        subject_kind: offer.kind_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    let encoded = reference.encode().unwrap();
    let hex: String = encoded.iter().map(|byte| format!("{byte:02x}")).collect();
    let kind = offer.kind_id.as_str().to_string();
    let (mut startup, mut profile) = crate::installed_browser::catalogs()?;
    for fixture in [
        if publish {
            test_json::source_offer()
        } else {
            test_json::reference_source_offer()
        },
        if publish {
            test_json::reference_sink_offer()
        } else {
            test_json::sink_offer()
        },
    ] {
        startup.insert(KindSignature {
            kind: fixture.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(KindDefinition {
                kind_id: fixture.kind_id.clone(),
                kind_contract_revision: fixture.kind_contract_revision.clone(),
                inputs: fixture.inputs.clone(),
                outputs: fixture.outputs.clone(),
                configuration: Vec::new(),
            })
            .unwrap();
        host.capabilities.push(fixture);
    }
    let source_kind = if publish {
        "conduit-test/json-source"
    } else {
        "conduit-test/resource-source"
    };
    let tail = if publish {
        "sink: conduit-test/resource-sink\n storage.value > sink.value"
    } else {
        "restore: todo/restore-summary\n sink: conduit-test/json-sink\n storage.value > restore.snapshot\n restore.result > sink.value"
    };
    let todo = include_str!("../../../../../forms/todo/main.conduit");
    let source = format!("{todo}\nform storage-proof {{\n source: {source_kind}\n storage: {kind}(reference = \"{hex}\")\n source.value > storage.value\n {tail}\n}}");
    let checked = conduit_form::check_syntax_document(
        &conduit_form::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, "storage-proof", &profile).unwrap();
    let hosts = [host];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let grants = if grant { vec![authority] } else { Vec::new() };
    let mut plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(debug_error)?;
    let fragment = plan.fragments.remove(0);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    validate_envelope(&fragment, &lowered, false)?;
    test_json::input(if publish { content } else { &encoded });
    Ok((prepare_scheduler(&fragment, &lowered)?, fragment))
}

#[test]
fn browser_todo_resource_requests_publish_then_restore_only_after_storage_acknowledges() {
    let content = super::json_tests::execute(
        r#"{"collection":[],"command":{"op":"append","value":{"complete":false,"text":"Milk"}}}"#,
        "todo/command-snapshot",
    )
    .unwrap();
    let (mut writer, fragment) = prepare(true, content.as_bytes(), true).unwrap();
    let DriveStatus::Effect(pending) = drive(&mut writer, &fragment).unwrap() else {
        panic!("missing publish")
    };
    let effect = resource_effect::describe(&writer, &pending).unwrap();
    assert_eq!(effect.effect_kind, "resource-publish");
    let key = effect.key.to_string();
    let record = effect.record.unwrap().to_vec();
    assert_eq!(writer.pending_host_operation_count(), 1);
    resource_effect::complete(&mut writer, &pending, Ok(None)).unwrap();
    assert!(resource_effect::complete(&mut writer, &pending, Ok(None)).is_err());
    let DriveStatus::Effect(output) = drive(&mut writer, &fragment).unwrap() else {
        panic!("missing published ResourceRef")
    };
    let BrowserHostEffect::Manifestation(ref value) = output.effect else {
        panic!("wrong ResourceRef effect")
    };
    let reference = BoundedResourceRef::decode(&value.canonical_value).unwrap();
    assert_eq!(reference.extent.bytes, content.len() as u64);
    complete_host_effect(&mut writer, &output).unwrap();
    assert!(matches!(
        drive(&mut writer, &fragment).unwrap(),
        DriveStatus::Complete
    ));
    let (mut reader, fragment) = prepare(false, content.as_bytes(), true).unwrap();
    let DriveStatus::Effect(pending) = drive(&mut reader, &fragment).unwrap() else {
        panic!("missing read")
    };
    let effect = resource_effect::describe(&reader, &pending).unwrap();
    assert_eq!(effect.key, key);
    assert_eq!(effect.effect_kind, "resource-read");
    resource_effect::complete(&mut reader, &pending, Ok(Some(&record))).unwrap();
    let DriveStatus::Effect(pending) = drive(&mut reader, &fragment).unwrap() else {
        panic!("missing summary")
    };
    let BrowserHostEffect::Manifestation(ref result) = pending.effect else {
        panic!("wrong result")
    };
    assert_eq!(result.canonical_value, br#"{"false":1,"total":1,"true":0}"#);
    complete_host_effect(&mut reader, &pending).unwrap();
    assert!(matches!(
        drive(&mut reader, &fragment).unwrap(),
        DriveStatus::Complete
    ));
}

#[test]
fn browser_resource_missing_authority_and_failed_storage_never_become_success() {
    assert!(prepare(true, b"[]", false).is_err());
    let (mut scheduler, fragment) = prepare(true, b"[]", true).unwrap();
    let DriveStatus::Effect(pending) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("missing publish")
    };
    resource_effect::complete(
        &mut scheduler,
        &pending,
        Err(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::HostOperationFailed,
            detail: 211,
        }),
    )
    .unwrap();
    assert!(
        matches!(drive(&mut scheduler, &fragment), Err(ref error) if error == "OperationFailed(211)")
    );
}
