use super::*;
use crate::{
    identity::BootIdentities,
    offer::{CpuFeatures, HostOffer},
};
use conduit_plan_lowering::lowering::lower_plan_fragment;

fn selected_bindings() -> ([NodeId; 5], [Option<HostCallBinding>; 5]) {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    let prepared = crate::tour_morse_plan::prepare(&identities, &offer, "build").unwrap();
    let fragment = &prepared.plan.fragments[0];
    let lowered = lower_plan_fragment(fragment).unwrap();
    let roles = [
        conduit_text::TEXT_LITERAL_KIND,
        conduit_text::TEXT_UPPER_KIND,
        conduit_semantic_catalog::TEXT_FACE_KIND,
        conduit_text::TEXT_MORSE_KIND,
        conduit_semantic_catalog::INDICATOR_FACE_KIND,
    ]
    .map(|kind| {
        NodeId(
            fragment
                .placements
                .iter()
                .position(|p| p.kind_id.as_str() == kind)
                .unwrap() as u16,
        )
    });
    let mut bindings = [None; 5];
    for operation in &lowered.host_calls {
        assert!(
            bindings[usize::from(operation.node.0)]
                .replace(operation.binding)
                .is_none()
        );
    }
    (roles, bindings)
}

#[test]
fn actual_morse_plan_retains_distinct_text_and_indicator_completion_contracts() {
    let (roles, bindings) = selected_bindings();
    let face = usize::from(roles[2].0);
    let indicator = usize::from(roles[4].0);
    assert_eq!(
        bindings[face].unwrap().maximum_output_bytes,
        conduit_core::MAX_FACE_COMPLETION_BYTES
    );
    assert_eq!(bindings[indicator].unwrap().maximum_output_bytes, 0);
    validate(roles, &bindings).unwrap();
    // The former native-only zero-output assumption rejects this actual Plan.
    let mut obsolete = bindings;
    obsolete[face].as_mut().unwrap().maximum_output_bytes = 0;
    assert_eq!(validate(roles, &obsolete), Err(SchedulerError::InvalidPlan));
    // A text completion allowance must not silently widen indicator authority.
    let mut widened_indicator = bindings;
    widened_indicator[indicator]
        .as_mut()
        .unwrap()
        .maximum_output_bytes = conduit_core::MAX_FACE_COMPLETION_BYTES;
    assert_eq!(
        validate(roles, &widened_indicator),
        Err(SchedulerError::InvalidPlan)
    );
}

#[test]
fn morse_contract_refuses_missing_foreign_or_misbound_host_calls() {
    let (roles, bindings) = selected_bindings();
    for role in &roles[1..] {
        let node = usize::from(role.0);
        let mut missing = bindings;
        missing[node] = None;
        assert!(validate(roles, &missing).is_err());
        let mut foreign = bindings;
        foreign[node].as_mut().unwrap().call = HostCallId(1);
        assert!(validate(roles, &foreign).is_err());
        let mut input = bindings;
        input[node].as_mut().unwrap().maximum_input_bytes += 1;
        assert!(validate(roles, &input).is_err());
        let mut output = bindings;
        output[node].as_mut().unwrap().maximum_output_bytes += 1;
        assert!(validate(roles, &output).is_err());
    }
    let mut literal = bindings;
    literal[usize::from(roles[0].0)] = bindings[usize::from(roles[1].0)];
    assert!(validate(roles, &literal).is_err());
    for index in 0..roles.len() {
        let mut aliased = roles;
        aliased[index] = roles[(index + 1) % roles.len()];
        assert!(validate(aliased, &bindings).is_err());
        let mut outside = roles;
        outside[index] = NodeId(u16::MAX);
        assert!(validate(outside, &bindings).is_err());
    }
}
