use super::*;
use crate::{kind_id, port_id};
use alloc::vec;

fn fixture() -> (
    Vec<PortDescriptor>,
    Vec<PortDescriptor>,
    KindSemanticContract,
) {
    let input = PortDescriptor {
        port_id: port_id("update"),
        value_kind: kind_id("test/update"),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: false },
        abnormal_kind: None,
    };
    let output = PortDescriptor {
        port_id: port_id("current"),
        value_kind: kind_id(crate::BOOL_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Current,
        abnormal_kind: None,
    };
    let law = InitializedStateLaw {
        update_port_id: input.port_id.clone(),
        current_port_id: output.port_id.clone(),
        current: CheckedValueContract::new(output.value_kind.clone(), 1, vec![]).unwrap(),
        initial: vec![0],
    };
    (
        vec![input],
        vec![output],
        KindSemanticContract {
            configuration: vec![],
            laws: vec![
                KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
                KindSemanticLaw::InitializedState(law),
            ],
        },
    )
}

#[test]
fn exact_heterogeneous_update_and_initial_current_are_checked() {
    let (inputs, outputs, semantic) = fixture();
    assert!(initialized_state_boundary(&inputs, &outputs, &semantic)
        .unwrap()
        .is_some());
    for mutation in 0..6 {
        let mut changed = semantic.clone();
        let KindSemanticLaw::InitializedState(law) = &mut changed.laws[1] else {
            unreachable!()
        };
        match mutation {
            0 => law.update_port_id = port_id("wrong"),
            1 => law.current_port_id = port_id("wrong"),
            2 => law.current.value_kind = kind_id("wrong"),
            3 => law.initial = vec![2],
            4 => law.current.maximum_bytes = 0,
            _ => law.initial = vec![0, 0],
        }
        assert!(initialized_state_boundary(&inputs, &outputs, &changed).is_err());
    }
    let mut duplicate = semantic.clone();
    duplicate.laws.push(duplicate.laws[1].clone());
    assert!(initialized_state_boundary(&inputs, &outputs, &duplicate).is_err());
}

#[test]
fn retained_without_initial_law_has_no_startup_cut() {
    let (inputs, outputs, mut semantic) = fixture();
    semantic.laws.pop();
    assert!(initialized_state_boundary(&inputs, &outputs, &semantic)
        .unwrap()
        .is_none());
}
