//! Exact source-resident selection only; no work/resource/Line authority issued.
use conduit_core as c;
use conduit_kernel::NodeId;
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope, selected_assigned_expression::*,
    selected_assigned_operation::AssignedOperationInput,
};
fn id(s: &str) -> c::AssignedIdentity {
    c::AssignedIdentity::from_text(s)
}
fn identity() -> ResidentExpressionIdentity {
    ResidentExpressionIdentity {
        kind: id("kind"),
        implementation: id("implementation"),
        artifact: id("artifact"),
        input_port: id("input"),
        input_kind: id("value/u64"),
        output_port: id("output"),
        output_kind: id("value/u64"),
        computation: c::AssignedComputationRequirement::from_text(0, "program", "00").unwrap(),
        private_work_units: 5,
    }
}
fn binding() -> c::AssignedRemoteBinding {
    c::AssignedRemoteBinding {
        line: id("line"),
        local_host: id("host"),
        local_boot: id("boot"),
        peer_host: id("peer"),
        peer_boot: id("peer-boot"),
    }
}
fn line(session: &str) -> c::AssignedLineForeRequirement {
    c::AssignedLineForeRequirement::new(
        c::AssignedForeRequirement::new(
            0,
            0,
            c::AssignedForeDirection::Input,
            id("front"),
            [7; 32],
        ),
        binding(),
        id(session),
        id("session-profile"),
        [8; 32],
    )
}
fn projection() -> Vec<u8> {
    let identity = identity();
    let mut out = vec![0; c::ASSIGNED_PLAN_HEADER_BYTES];
    out[..8].copy_from_slice(&c::assigned_plan_magic());
    out[8..10].copy_from_slice(&c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA.to_le_bytes());
    out[12..14].copy_from_slice(&128u16.to_le_bytes());
    for (offset, name) in [(16, "plan"), (32, "fragment"), (48, "host"), (64, "boot")] {
        out[offset..offset + 16].copy_from_slice(&id(name).0);
    }
    let mut row = |tag: u8, payload: &[u8]| {
        out[80 + usize::from(tag - 1)] += 1;
        out.push(tag);
        out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        out.extend_from_slice(payload);
    };
    let mut node = [0; 52];
    node[2..4].copy_from_slice(&8u16.to_le_bytes());
    node[4..20].copy_from_slice(&identity.kind.0);
    node[20..36].copy_from_slice(&identity.implementation.0);
    node[36..52].copy_from_slice(&identity.artifact.0);
    row(c::ASSIGNED_NODE, &node);
    for (direction, port) in [(0, identity.input_port), (1, identity.output_port)] {
        let mut bytes = [0; 37];
        bytes[4] = direction;
        bytes[5..21].copy_from_slice(&port.0);
        bytes[21..37].copy_from_slice(&id("value/u64").0);
        row(c::ASSIGNED_PORT, &bytes);
    }
    let mut config = [0; 55];
    config[2..18].copy_from_slice(&identity.computation.key.0);
    config[18] = 4;
    config[19..23].copy_from_slice(&identity.computation.text_bytes.to_le_bytes());
    config[23..].copy_from_slice(&identity.computation.text_digest);
    row(c::ASSIGNED_CONFIGURATION, &config);
    let mut resource = [0; 8];
    resource[4..].copy_from_slice(&5u32.to_le_bytes());
    row(c::ASSIGNED_RESOURCE, &resource);
    row(c::ASSIGNED_REMOTE_ENDPOINT, line("session").payload());
    let len = out.len() as u16;
    out[10..12].copy_from_slice(&len.to_le_bytes());
    let digest = c::assigned_plan_payload_digest(&out[125..]);
    out[93..125].copy_from_slice(&digest);
    out
}
fn selected<'a>(
    input: AssignedOperationInput<'a>,
    active: &'a c::ActivePlayIdentity,
    computations: &'a [c::AssignedComputationRequirement],
    bindings: &'a [c::AssignedRemoteBinding],
) -> SelectedAssignedExpressionPlan<'a> {
    SelectedAssignedExpressionPlan {
        input,
        maxima: c::AssignedPlanMaxima {
            encoded_bytes: 4096,
            runtime_state_bytes: 4096,
            counts: [16; 13],
        },
        requirements: c::AssignedPlanRequirements {
            host: id("host"),
            boot: id("boot"),
            host_calls: &[],
            resources: &[0],
            remote_bindings: bindings,
        },
        computations: c::AssignedComputationProfile {
            maximum_text_bytes: 2,
            maximum_total_text_bytes: 2,
            requirements: computations,
        },
        fore: None,
        fragment_id: "fragment",
        scope: AssignedPlayScope::Plot(active),
        node: NodeId(0),
        work_resource: 0,
    }
}
#[test]
fn resident_line_profile_is_explicit_and_every_original_source_pin_still_checks() {
    let bytes = projection();
    let active = c::bind_active_play(&"plan".into(), &"host".into(), &"boot".into(), 1);
    let computations = [identity().computation];
    let bindings = [binding()];
    let lines = [line("session")];
    let fore = c::AssignedLineForeProfile {
        local: &[],
        lines: &lines,
    };
    let check = |identity, active: &c::ActivePlayIdentity, fore| {
        check_selected_resident_expression_with_line_fore(
            identity,
            SelectedAssignedLineForeExpressionPlan {
                expression: selected(
                    AssignedOperationInput::Contiguous(&bytes),
                    active,
                    &computations,
                    &bindings,
                ),
                fore,
            },
        )
    };
    assert_eq!(check(identity(), &active, fore), Ok(NodeId(0)));
    assert!(check_selected_resident_expression(
        identity(),
        selected(
            AssignedOperationInput::Contiguous(&bytes),
            &active,
            &computations,
            &bindings
        )
    )
    .is_err());
    for changed in 0..3 {
        let mut source = identity();
        match changed {
            0 => source.artifact = id("foreign-artifact"),
            1 => source.computation.text_digest = [4; 32],
            _ => source.private_work_units = 4,
        };
        assert!(check(source, &active, fore).is_err());
    }
    let mut foreign = active.clone();
    foreign.boot_id = "foreign-boot".into();
    assert!(check(identity(), &foreign, fore).is_err());
    let foreign = [line("foreign-session")];
    assert!(check(
        identity(),
        &active,
        c::AssignedLineForeProfile {
            local: &[],
            lines: &foreign
        }
    )
    .is_err());
    let records: Vec<_> = c::AssignedPlanRecords::new(&bytes)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(check_selected_resident_expression_with_line_fore(
        identity(),
        SelectedAssignedLineForeExpressionPlan {
            expression: selected(
                AssignedOperationInput::Records {
                    header: &bytes[..125],
                    records: &records
                },
                &active,
                &computations,
                &bindings
            ),
            fore
        }
    )
    .is_err());
    let mut expression = selected(
        AssignedOperationInput::Contiguous(&bytes),
        &active,
        &computations,
        &bindings,
    );
    expression.fore = Some(&[]);
    assert!(check_selected_resident_expression_with_line_fore(
        identity(),
        SelectedAssignedLineForeExpressionPlan { expression, fore }
    )
    .is_err());
}
