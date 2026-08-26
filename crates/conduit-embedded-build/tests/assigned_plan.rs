use conduit_core::{
    assigned_plan_payload_digest, decode_assigned_plan, AssignedIdentity, AssignedPlanMaxima,
    AssignedPlanRefusal, AssignedPlanRequirements, AssignedRemoteBinding, BootId, ConnectionBase,
    ASSIGNED_PLAN_HEADER_BYTES,
};
use conduit_embedded_build::{
    encode_assigned_plan, generate_embedded_plan, EmbeddedImageBounds, GeneratedEmbeddedPlan,
    GenerationError,
};
use conduit_runtime::lowering::lower_plan_fragment;
use conduit_signal::{signal_profile_catalog, SIGNAL_ENCODED_LEN};
use conduit_signal_conformance::{pico_local_advertisement, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS};
use conduit_system_continuity::{exact_r1_signal_plan, R1SignalRouteSet};

#[test]
fn local_assigned_plan_has_a_deterministic_golden_round_trip() {
    let generated = local_signal_plan();
    let first = encode_assigned_plan(&generated, AssignedPlanMaxima::TINY_HOST).unwrap();
    let second = encode_assigned_plan(&generated, AssignedPlanMaxima::TINY_HOST).unwrap();
    assert_eq!(first, second);

    let operations = operation_requirements(&generated);
    let resources = resource_requirements(&generated);
    let view = decode_assigned_plan(
        &first,
        AssignedPlanMaxima::TINY_HOST,
        requirements(&generated, &operations, &resources, &[]),
    )
    .unwrap();
    assert_eq!(view.encoded_bytes as usize, first.len());
    assert!(view.encoded_bytes + view.runtime_state_bytes <= 2_560);
    assert_eq!(
        assigned_plan_payload_digest(&first),
        [
            77, 144, 219, 180, 124, 193, 174, 220, 117, 194, 210, 35, 51, 113, 210, 193, 101, 94,
            198, 11, 89, 111, 119, 174, 229, 254, 6, 245, 168, 127, 105, 2,
        ],
        "local assigned-plan golden changed"
    );
    assert_excludes_global_truth(&first, &generated);
}

#[test]
fn one_remote_assigned_plan_has_a_deterministic_golden_round_trip() {
    let generated = remote_signal_plan();
    let bytes = encode_assigned_plan(&generated, AssignedPlanMaxima::TINY_HOST).unwrap();
    let operations = operation_requirements(&generated);
    let resources = resource_requirements(&generated);
    let remotes = remote_requirements(&generated);
    let view = decode_assigned_plan(
        &bytes,
        AssignedPlanMaxima::TINY_HOST,
        requirements(&generated, &operations, &resources, &remotes),
    )
    .unwrap();
    assert!(view.encoded_bytes + view.runtime_state_bytes <= 2_560);
    assert_eq!(
        assigned_plan_payload_digest(&bytes),
        [
            245, 132, 2, 21, 202, 162, 59, 161, 83, 3, 30, 56, 94, 202, 64, 81, 91, 55, 57, 79, 89,
            237, 248, 38, 2, 224, 100, 60, 235, 134, 18, 142,
        ],
        "remote assigned-plan golden changed"
    );
    assert_excludes_global_truth(&bytes, &generated);
}

#[test]
fn assigned_plan_refuses_identity_inventory_capacity_and_global_mutations() {
    let local = local_signal_plan();
    let bytes = encode_assigned_plan(&local, AssignedPlanMaxima::TINY_HOST).unwrap();
    let operations = operation_requirements(&local);
    let resources = resource_requirements(&local);
    let valid = requirements(&local, &operations, &resources, &[]);

    assert_eq!(
        decode_assigned_plan(
            &bytes,
            AssignedPlanMaxima::TINY_HOST,
            AssignedPlanRequirements {
                host: AssignedIdentity::from_text("wrong-host"),
                ..valid
            }
        ),
        Err(AssignedPlanRefusal::WrongHost)
    );
    assert_eq!(
        decode_assigned_plan(
            &bytes,
            AssignedPlanMaxima::TINY_HOST,
            AssignedPlanRequirements {
                boot: AssignedIdentity::from_text("stale-boot"),
                ..valid
            }
        ),
        Err(AssignedPlanRefusal::WrongBoot)
    );

    let missing_resource = remove_first_record(&bytes, conduit_core::ASSIGNED_RESOURCE);
    assert_eq!(
        decode_assigned_plan(&missing_resource, AssignedPlanMaxima::TINY_HOST, valid),
        Err(AssignedPlanRefusal::MissingResource)
    );
    let unknown_operation = [AssignedIdentity::from_text("operation/not-assigned")];
    assert_eq!(
        decode_assigned_plan(
            &bytes,
            AssignedPlanMaxima::TINY_HOST,
            requirements(&local, &unknown_operation, &resources, &[])
        ),
        Err(AssignedPlanRefusal::UnknownOperation)
    );

    let mut route_maxima = AssignedPlanMaxima::TINY_HOST;
    route_maxima.counts[4] = 0;
    assert!(matches!(
        encode_assigned_plan(&local, route_maxima),
        Err(GenerationError::BoundExceeded { .. })
    ));
    let mut byte_maxima = AssignedPlanMaxima::TINY_HOST;
    byte_maxima.encoded_bytes = 128;
    assert!(matches!(
        encode_assigned_plan(&local, byte_maxima),
        Err(GenerationError::BoundExceeded { .. })
    ));

    let remote = remote_signal_plan();
    let remote_bytes = encode_assigned_plan(&remote, AssignedPlanMaxima::TINY_HOST).unwrap();
    let remote_operations = operation_requirements(&remote);
    let remote_resources = resource_requirements(&remote);
    let mut stale = remote_requirements(&remote);
    stale[0].peer_boot = AssignedIdentity::from_text("rebooted-peer");
    assert_eq!(
        decode_assigned_plan(
            &remote_bytes,
            AssignedPlanMaxima::TINY_HOST,
            requirements(&remote, &remote_operations, &remote_resources, &stale)
        ),
        Err(AssignedPlanRefusal::StaleRemoteEndpoint)
    );

    let mut global_record = bytes.clone();
    global_record.extend_from_slice(&[250, 1, 0, 1]);
    let new_len = u16::try_from(global_record.len()).unwrap();
    global_record[10..12].copy_from_slice(&new_len.to_le_bytes());
    let digest = assigned_plan_payload_digest(&global_record[ASSIGNED_PLAN_HEADER_BYTES..]);
    global_record[92..124].copy_from_slice(&digest);
    assert_eq!(
        decode_assigned_plan(&global_record, AssignedPlanMaxima::TINY_HOST, valid),
        Err(AssignedPlanRefusal::UnknownRecord(250))
    );
}

fn local_signal_plan() -> GeneratedEmbeddedPlan {
    let form = conduit_form::parse_with_startup(
        include_str!("../../../fixtures/forms/signal-demo.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .unwrap();
    let host = pico_local_advertisement();
    let placements =
        conduit_planner::default_placements(&form, std::slice::from_ref(&host)).unwrap();
    let plan = conduit_planner::plan_with_connection_limits(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
    )
    .unwrap();
    generated_for(&plan.fragments[0])
}

fn remote_signal_plan() -> GeneratedEmbeddedPlan {
    let exact = exact_r1_signal_plan(
        BootId::from(conduit_net::R1_PICO_BOOT_ID),
        R1SignalRouteSet::UsbOnly,
    )
    .unwrap();
    let fragment = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == conduit_net::R1_PICO_HOST_ID)
        .unwrap();
    generated_for(fragment)
}

fn generated_for(fragment: &conduit_core::PlanFragment) -> GeneratedEmbeddedPlan {
    let lowered = lower_plan_fragment(fragment).unwrap();
    generate_embedded_plan(fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING).unwrap()
}

fn operation_requirements(plan: &GeneratedEmbeddedPlan) -> Vec<AssignedIdentity> {
    plan.host_operations
        .iter()
        .map(|item| AssignedIdentity::from_text(&item.contract_id))
        .collect()
}

fn resource_requirements(plan: &GeneratedEmbeddedPlan) -> Vec<u16> {
    plan.resources.iter().map(|item| item.resource).collect()
}

fn remote_requirements(plan: &GeneratedEmbeddedPlan) -> Vec<AssignedRemoteBinding> {
    plan.remote_endpoints
        .iter()
        .map(|item| AssignedRemoteBinding {
            line: AssignedIdentity::from_text(&item.line_id),
            local_host: AssignedIdentity::from_text(&item.local_host),
            local_boot: AssignedIdentity::from_text(&item.local_boot),
            peer_host: AssignedIdentity::from_text(&item.peer_host),
            peer_boot: AssignedIdentity::from_text(&item.peer_boot),
        })
        .collect()
}

fn requirements<'a>(
    plan: &GeneratedEmbeddedPlan,
    operations: &'a [AssignedIdentity],
    resources: &'a [u16],
    remotes: &'a [AssignedRemoteBinding],
) -> AssignedPlanRequirements<'a> {
    AssignedPlanRequirements {
        host: AssignedIdentity::from_text(&plan.host_id),
        boot: AssignedIdentity::from_text(&plan.boot_id),
        operations,
        resources,
        remote_bindings: remotes,
    }
}

fn assert_excludes_global_truth(bytes: &[u8], plan: &GeneratedEmbeddedPlan) {
    for forbidden in [
        "source/",
        "checked/",
        "expanded/",
        "ExecutionPlan",
        "Presentation",
        "Body",
    ] {
        assert!(!bytes
            .windows(forbidden.len())
            .any(|window| window == forbidden.as_bytes()));
    }
    assert!(!bytes
        .windows(plan.plan_id.len())
        .any(|window| window == plan.plan_id.as_bytes()));
}

fn remove_first_record(bytes: &[u8], wanted: u8) -> Vec<u8> {
    let mut output = bytes.to_vec();
    let mut cursor = ASSIGNED_PLAN_HEADER_BYTES;
    while cursor < output.len() {
        let tag = output[cursor];
        let length = usize::from(u16::from_le_bytes([output[cursor + 1], output[cursor + 2]]));
        let end = cursor + 3 + length;
        if tag == wanted {
            output.drain(cursor..end);
            output[80 + usize::from(tag - 1)] -= 1;
            let new_len = u16::try_from(output.len()).unwrap();
            output[10..12].copy_from_slice(&new_len.to_le_bytes());
            let digest = assigned_plan_payload_digest(&output[ASSIGNED_PLAN_HEADER_BYTES..]);
            output[92..124].copy_from_slice(&digest);
            return output;
        }
        cursor = end;
    }
    panic!("wanted assigned-plan record was absent")
}
