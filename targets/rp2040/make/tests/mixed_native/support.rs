#![allow(dead_code)] // Shared construction helpers also compile in the outer test.
use conduit_core::*;
use conduit_embedded_build as e;
use conduit_i2c_transaction::{contract::*, installation::*};
use conduit_plot as p;
#[path = "authority.rs"]
pub(crate) mod authority;
#[path = "../../../../../mechanisms/io/i2c-transaction/src/test_support.rs"]
mod selected_fixture;

pub fn context_for(
    outcome: bool,
) -> (
    p::CheckedSyntaxDocument,
    p::PortableExpressionProgram,
    e::PreparedResidentExpression,
) {
    let types = p::check_syntax_document(
        &p::parse_syntax_document(include_str!(
            "../../../../../plots/device-protocols/i2c-types.conduit"
        )),
        &p::StartupCatalog::new(),
    )
    .unwrap();
    let mut catalog = p::StartupCatalog::new();
    for (name, path) in [
        ("I2cTransaction", "machine/i2c/transact/request"),
        ("I2cResult", "machine/i2c/transact/result"),
    ] {
        let ty = types.native_types.iter().find(|t| t.name == name).unwrap();
        catalog
            .insert_checked_native_type_from_document(path, &types, &ty.identity)
            .unwrap();
    }
    let document = p::check_syntax_document(
        &p::parse_syntax_document(if outcome {
            include_str!("../../../../../plots/device-protocols/ht16k33.conduit")
        } else {
            include_str!("request.conduit")
        }),
        &catalog,
    )
    .unwrap();
    let expanded = p::expand_canonical_plot_for_authoring(
        &document,
        if outcome {
            "ht16k33-write-outcome"
        } else {
            "request"
        },
        &p::ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(text) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let program = p::PortableExpressionProgram::from_canonical_hex(text).unwrap();
    let image = e::PreparedResidentExpression::prepare(
        &document,
        &program,
        PortTemporal::Flow { closes: true },
        e::ResidentExpressionBounds {
            native: p::rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            maximum_occurrences: 4096,
            maximum_program_bytes: 32768,
            maximum_generated_bytes: 1024 * 1024,
            private_work_units: 1000,
        },
    )
    .unwrap();
    (document, program, image)
}

pub fn model(
    document: &p::CheckedSyntaxDocument,
    program: &p::PortableExpressionProgram,
    image: &e::PreparedResidentExpression,
    scope: &BaseCapabilityScope,
    contract: &I2cContract,
    reverse: bool,
) -> Plan {
    let (mut f, _, _, _) = selected_fixture::selected(
        scope,
        contract.kind(),
        I2C_CALL,
        I2C_MAXIMUM_BYTES,
        "i2c",
        "i2c",
    );
    f.source_document_id = document.source_document_id.clone();
    f.placements[0].artifact_id = "fixture/image".into();
    f.placements[0].execution_profile_id = I2C_EXECUTION_PROFILE.into();
    f.placements[0].base.as_mut().unwrap().implementation_id =
        "rp2040/i2c@1".into();
    let mut resident = f.placements[0].clone();
    let offer = image.offer_description();
    resident.placement_id = "placement/resident".into();
    resident.gear_id = "gear/resident".into();
    resident.kind_id = offer.kind_id.clone();
    resident.kind_contract_revision = offer.kind_contract_revision.clone();
    resident.capability_id = offer.capability_id.clone();
    resident.execution_profile_id = offer.implementation.execution_profile_id.clone();
    resident.implementation_id = offer.implementation.implementation_id.clone();
    resident.artifact_id = image.artifact_id().clone();
    resident.inputs = offer.inputs.clone();
    resident.outputs = offer.outputs.clone();
    resident.semantic_contract = offer.semantic_contract.clone();
    resident.limits = offer.limits.clone();
    resident.base = None;
    resident.host_calls.clear();
    resident.authority.clear();
    resident.configuration = vec![ConfigurationEntry {
        key: "program".into(),
        value: ConfigurationValue::Text(program.canonical_hex().unwrap()),
    }];
    resident.resources = vec![ResourceBinding {
        pool_id: "fixture/private-work".into(),
        class_id: COOPERATIVE_STEP_WORK_CLASS_ID.into(),
        units: 1000,
        protected: None,
        compute: None,
        content: None,
    }];
    let native_first = resident.inputs[0].value_kind == f.placements[0].outputs[0].value_kind;
    let (first, last) = if native_first {
        (&f.placements[0], &resident)
    } else {
        (&resident, &f.placements[0])
    };
    f.connections = vec![PlannedConnection {
        connection_id: "connection/request".into(),
        source_placement_id: first.placement_id.clone(),
        source_port_id: first.outputs[0].port_id.clone(),
        sink_placement_id: last.placement_id.clone(),
        sink_port_id: last.inputs[0].port_id.clone(),
        value_kind: first.outputs[0].value_kind.clone(),
        resource: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Flow { closes: true },
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        selected_line: None,
        admitted_lines: vec![],
        item_capacity: 1,
        byte_capacity: I2C_MAXIMUM_BYTES,
    }];
    for (direction, gear, port) in [
        (PortDirection::Input, first, &first.inputs[0]),
        (PortDirection::Output, last, &last.outputs[0]),
    ] {
        f.fore_ports.push(PlannedForePort {
            front_port_id: port.port_id.clone(),
            direction,
            placement_id: gear.placement_id.clone(),
            gear_port_id: port.port_id.clone(),
            value_kind: port.value_kind.clone(),
            value_contract: gear
                .checked_port_front()
                .value_contract(&match direction {
                    PortDirection::Input => FrontValueLocation::Input(port.port_id.clone()),
                    PortDirection::Output => FrontValueLocation::Output(port.port_id.clone()),
                })
                .cloned(),
            abnormal_kind: None,
            track: ConnectionTrack::Payload,
            temporal: port.temporal,
            pressure_policy: DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: I2C_MAXIMUM_BYTES,
            selected_line: None,
        });
    }
    if native_first {
        f.startup_order.push(resident.placement_id.clone());
    } else {
        f.startup_order.insert(0, resident.placement_id.clone());
    }
    f.expected_sign.push(ExpectedSign::PlacementPrepared(
        resident.placement_id.clone(),
    ));
    f.expected_sign.push(ExpectedSign::PlacementTerminal(
        resident.placement_id.clone(),
    ));
    f.expected_sign.push(ExpectedSign::ConnectionTerminal(
        "connection/request".into(),
    ));
    f.sign_storage_budget = mandatory_sign_storage_requirement(&f.expected_sign).unwrap();
    f.placements.insert(0, resident);
    if reverse {
        f.placements.swap(0, 1);
    }
    seal_plan(
        PlotIdentity {
            source_document_id: f.source_document_id.clone(),
            checked_plot_id: f.checked_plot_id.clone(),
            expanded_plot_id: f.expanded_plot_id.clone(),
        },
        vec![f],
    )
}

pub fn context() -> (
    p::CheckedSyntaxDocument,
    p::PortableExpressionProgram,
    e::PreparedResidentExpression,
) {
    context_for(false)
}
