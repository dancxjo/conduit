//! Exact authored catalogs, independent Root selections and sealed fixture Plans.
use super::*;
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn reference() -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([41; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: conduit_std_offers::JSON_SNAPSHOT_ACCESS_CLASS.into(),
        extent: ResourceExtent {
            bytes: JSON.len() as u64,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([42; 32]),
            expires_at: None,
        },
    }
}
pub(super) fn host_id() -> HostId {
    "h".repeat(conduit_std_offers::JSON_SNAPSHOT_MAXIMUM_ID_BYTES)
        .into()
}
pub(super) fn selected(root: &Path, boot: &str, write: bool) -> SelectedJsonSnapshotResidence {
    let reference = reference();
    // Explicit fixture Root authorization, independently retained from any Plan.
    let original = AuthorityGrant {
        grant_id: (if write { "p" } else { "r" })
            .repeat(conduit_std_offers::JSON_SNAPSHOT_MAXIMUM_ID_BYTES)
            .into(),
        contract_id: conduit_std_offers::JSON_SNAPSHOT_AUTHORITY.into(),
        host_call_contract_id: if write {
            conduit_std_offers::JSON_SNAPSHOT_PUBLISH_CALL
        } else {
            conduit_std_offers::JSON_SNAPSHOT_READ_CALL
        }
        .into(),
        subject_kind: if write {
            conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND
        } else {
            conduit_semantic_catalog::SNAPSHOT_READ_KIND
        }
        .into(),
        host_id: host_id(),
        boot_id: boot.into(),
        capability_id: if write {
            conduit_std_offers::JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION
        } else {
            conduit_std_offers::JSON_SNAPSHOT_READ_IMPLEMENTATION
        }
        .into(),
    };
    let local = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "h"
            .repeat(conduit_std_offers::JSON_SNAPSHOT_MAXIMUM_ID_BYTES)
            .into(),
        authority_contract: original.contract_id.clone(),
        authority_grant: original.grant_id.clone(),
        maximum_bytes: 4096,
        maximum_items: Some(1),
        availability: ResourceReferenceAvailability::Available,
    };
    SelectedJsonSnapshotResidence::prepare(File::open(root).unwrap(), reference, original, local)
        .unwrap()
}
pub(super) fn optional() -> (StructuredInfoType, StructuredInfoValue) {
    let ty =
        optional_info_type(StructuredInfoType::leaf(kind_id(RESOURCE_REFERENCE_INFO_ID)).unwrap())
            .unwrap();
    let empty = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap(),
        vec![],
    )
    .unwrap();
    let none = StructuredInfoValue::variant(ty.clone(), "none", empty).unwrap();
    (ty, none)
}
#[derive(Clone, Copy)]
pub(super) enum Mode {
    Publish,
    Recover,
    Read,
}
pub(super) fn fixture(
    root: &Path,
    boot: &str,
    mode: Mode,
) -> (StdHost, PlanFragment, SelectedJsonSnapshotResidence) {
    let selected = selected(root, boot, matches!(mode, Mode::Publish));
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: host_id(),
        boot_id: boot.into(),
        offer_generation: OfferGeneration(1),
    });
    selected.install_offer(&mut host).unwrap();
    let (ty, none) = optional();
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", kind_id(RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    startup
        .insert_value_kind_alias("JsonText", kind_id(conduit_web::JSON_TEXT_INFO_ID))
        .unwrap();
    startup
        .insert_structured_type("OptionalReference", ty.clone())
        .unwrap();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_resource_snapshot_catalogs(&mut startup, &mut profiles)
        .unwrap();
    conduit_semantic_catalog::state_value::install_state_value_kind(
        "OptionalReference",
        &ty,
        &none,
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let optional_maximum = u32::try_from(
        none.canonical_bytes().unwrap().len() + MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES,
    )
    .unwrap();
    let value_contract = CheckedValueContract::new(
        ty.profile().unwrap().value_kind().clone(),
        optional_maximum,
        vec![],
    )
    .unwrap();
    let trigger_contract = CheckedValueContract::new(kind_id("value/u64"), 8, vec![]).unwrap();
    let reference_contract = CheckedValueContract::new(
        kind_id(RESOURCE_REFERENCE_INFO_ID),
        MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
        vec![],
    )
    .unwrap();
    let overflow_contract = CheckedValueContract::new(kind_id(EMPTY_INFO_ID), 0, vec![]).unwrap();
    if matches!(mode, Mode::Read) {
        startup
            .insert_value_kind_alias("ReadTrigger", kind_id("value/u64"))
            .unwrap();
        startup
            .insert_structured_type(
                "CollectedReferences",
                StructuredInfoType::sequence(
                    StructuredInfoType::leaf(kind_id(RESOURCE_REFERENCE_INFO_ID)).unwrap(),
                    1,
                )
                .unwrap(),
            )
            .unwrap();
        conduit_semantic_catalog::install_current_sample_finite_kind(
            &value_contract,
            &trigger_contract,
            &mut startup,
            &mut profiles,
        )
        .unwrap();
        conduit_semantic_catalog::install_flow_collect_kind(
            &reference_contract,
            1,
            &overflow_contract,
            &mut startup,
            &mut profiles,
        )
        .unwrap();
        host.install_finite_current_sampling(&value_contract, &trigger_contract)
            .unwrap();
        host.advertisement
            .capabilities
            .push(conduit_std_offers::flow_collect_offer(&reference_contract, 1).unwrap());
    }
    let encoded = hex(&reference().encode().unwrap());
    let source = match mode {
        Mode::Publish => format!("plot wrap (\n >> reference: ResourceRef\n wrapped: OptionalReference >>\n) = (some(.))\nplot recorded (\n >> json: JsonText\n current: $OptionalReference >>\n) {{\n publish: resource/publish-json-snapshot(\"{encoded}\")\n cell: keep ResourceRef? for life\n wrapped: wrap\n json >> publish.source\n publish.value >> wrapped.reference\n wrapped.wrapped >> cell\n cell >> current\n}}\n"),
        Mode::Recover => "plot recorded (\n current: $OptionalReference >>\n) {\n cell: keep ResourceRef? for life\n cell >> current\n}\n".into(),
        Mode::Read => format!("plot overflow-flag (\n >> terminal: Empty\n flag: Boolean >>\n) = (true)\nplot unwrap (\n >> optional: OptionalReference...|\n reference: ResourceRef...| >>\n) = (.some)\nplot exactly-one (\n >> values: CollectedReferences\n reference: ResourceRef >>\n) = (sequence/at(., 0))\nplot recorded (\n >> trigger: ReadTrigger...| <= 8B\n json: JsonText >>\n overflow: Boolean >>\n) {{\n cell: keep ResourceRef? for life\n sample: current/sample/finite\n unwrap-reference: unwrap\n collect: flow/collect\n refused: overflow-flag\n one: exactly-one\n read: resource/read-json-snapshot(\"{encoded}\")\n cell >> sample.current\n trigger >> sample.trigger\n sample.value >> unwrap-reference.optional\n unwrap-reference.reference >> collect.values\n collect.collected >> one.values\n collect.collected! >> refused.terminal\n refused.flag >> overflow\n one.reference >> read.source\n read.value >> json\n}}\n"),
    };
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "recorded", &profiles).unwrap();
    host.advertisement.capabilities.retain(|offer| {
        offer.kind_id.as_str() != conduit_semantic_catalog::state_value::STATE_VALUE_KIND
    });
    {
        host.advertisement.capabilities.push(
            conduit_std_offers::state_value_durable_std_offer("OptionalReference", &ty, &none)
                .unwrap(),
        );
        host.advertisement.resources.push(resource_offer(
            "json/state",
            conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS,
            1,
        ));
    }
    for gear in &expanded.expanded.gears {
        if let Some(ConfigurationEntry {
            value: ConfigurationValue::Text(code),
            ..
        }) = gear
            .configuration
            .iter()
            .find(|entry| entry.key == "program")
        {
            let program = PortableExpressionProgram::from_canonical_hex(code).unwrap();
            assert_eq!(
                program.output_type,
                if matches!(program.input_type.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == EMPTY_INFO_ID)
                {
                    StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap()
                } else if matches!(mode, Mode::Read) {
                    StructuredInfoType::leaf(kind_id(RESOURCE_REFERENCE_INFO_ID)).unwrap()
                } else {
                    ty.clone()
                },
                "ordinary constructor/extractor must preserve exact canonical output Type"
            );
            host.advertisement.capabilities.push(
                conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                    .unwrap(),
            );
        }
    }
    for gear in &expanded.expanded.gears {
        if let Some(ConfigurationEntry {
            value: ConfigurationValue::Text(code),
            ..
        }) = gear
            .configuration
            .iter()
            .find(|entry| entry.key == "selector")
        {
            let selector = StructuredSelector::from_canonical_hex(code).unwrap();
            host.advertisement.capabilities.push(
                conduit_std_offers::structured_selector_std_offer(
                    &selector,
                    gear.inputs[0].temporal,
                ),
            );
        }
    }
    host.advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    host.advertisement
        .resources
        .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(host.advertisement()).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let mut boundaries = BTreeMap::new();
    let ports = match mode {
        Mode::Publish => vec![
            ("json", PortDirection::Input),
            ("current", PortDirection::Output),
        ],
        Mode::Recover => vec![("current", PortDirection::Output)],
        Mode::Read => vec![
            ("trigger", PortDirection::Input),
            ("json", PortDirection::Output),
            ("overflow", PortDirection::Output),
        ],
    };
    for (name, direction) in ports {
        boundaries.insert(
            ForeBoundaryKey {
                direction,
                front_port_id: port_id(name),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: match name {
                    "trigger" => 8,
                    "overflow" => 1,
                    _ => 4096,
                },
            },
        );
    }
    let optional_kind = ty.profile().unwrap().value_kind().clone();
    let sequence_type = StructuredInfoType::sequence(
        StructuredInfoType::leaf(kind_id(RESOURCE_REFERENCE_INFO_ID)).unwrap(),
        1,
    )
    .unwrap();
    let sequence_kind = sequence_type.profile().unwrap().value_kind().clone();
    let sequence_maximum = PreparedLeafSequenceEncoder::new(
        kind_id(RESOURCE_REFERENCE_INFO_ID),
        MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
        1,
    )
    .unwrap()
    .maximum_bytes();
    let mut connections = BTreeMap::new();
    for cord in &expanded.expanded.connections {
        let source = expanded
            .expanded
            .gears
            .iter()
            .find(|gear| gear.gear_id == cord.source_gear_id)
            .unwrap();
        let port = source
            .outputs
            .iter()
            .find(|port| port.port_id == cord.source_port_id)
            .unwrap();
        let byte_capacity = if cord.track == ConnectionTrack::AbnormalTerminal {
            1
        } else if port.value_kind == optional_kind {
            optional_maximum
        } else if port.value_kind == sequence_kind {
            sequence_maximum
        } else {
            match port.value_kind.as_str() {
                RESOURCE_REFERENCE_INFO_ID => MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
                conduit_web::JSON_TEXT_INFO_ID => conduit_std_offers::JSON_SNAPSHOT_MAXIMUM_BYTES,
                "value/u64" => 8,
                BOOL_INFO_ID => 1,
                EMPTY_INFO_ID => 1,
                unknown => panic!("fixture has no reviewed exact payload bound for {unknown}"),
            }
        };
        connections.insert(
            (
                cord.source_gear_id.clone(),
                cord.source_port_id.clone(),
                cord.sink_gear_id.clone(),
                cord.sink_port_id.clone(),
            ),
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity,
            },
        );
    }
    let empty = BTreeMap::new();
    let plan = plan_expanded_authoring_with_connection_limits(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: std::slice::from_ref(selected.authority()),
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &connections,
        &boundaries,
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    (host, plan.fragments[0].clone(), selected)
}
