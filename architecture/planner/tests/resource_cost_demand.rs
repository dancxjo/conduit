//! Equal byte length is not equal work. Validate actual typed input custody.
use conduit_core::*;
use conduit_planner::resource_cost::*;
use conduit_plot::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};

fn checked_text_work() -> conduit_plot::CheckedGear {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "fixture/text-work".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let port = |name: &str, direction| PortDescriptor {
        port_id: name.into(),
        value_kind: TEXT_INFO_ID.into(),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert(KindProjection {
            kind_id: "fixture/text-work".into(),
            kind_contract_revision: "fixture/text-work@1".into(),
            inputs: vec![port("text", PortDirection::Input)],
            outputs: vec![port("out", PortDirection::Output)],
            configuration: vec![],
        })
        .unwrap();
    conduit_plot::parse_with_startup(
        "plot work {\n job: fixture/text-work\n}\n",
        &startup,
        &profiles,
    )
    .unwrap()
    .gears
    .remove(0)
}
#[test]
fn equal_size_different_input_cannot_reuse_a_favorable_forecast() {
    let gear = checked_text_work();
    let port = &gear.inputs[0].port_id;
    let first = ValuePayload {
        value_kind: TEXT_INFO_ID.into(),
        encoded: b"private prompt one".to_vec(),
    };
    let second = ValuePayload {
        value_kind: TEXT_INFO_ID.into(),
        encoded: b"private prompt two".to_vec(),
    };
    let first_inputs = [CostInput {
        port,
        value: &first,
    }];
    let second_inputs = [CostInput {
        port,
        value: &second,
    }];
    let first_demand = CostDemand::admit(&gear, &first_inputs).unwrap();
    let second_demand = CostDemand::admit(&gear, &second_inputs).unwrap();
    assert_eq!(first_demand.input_bytes(), second_demand.input_bytes());
    assert_eq!(
        first_demand.validate_match(second_demand),
        Err(CostEvidenceRefusal::WorkExtentChanged)
    );
    assert_eq!(first_demand.validate_match(first_demand), Ok(()));
    let debug = format!("{first_demand:?}");
    assert!(debug.contains("input_bytes"));
    assert!(!debug.contains("private"));
    assert!(!debug.contains("112, 114"));
}
#[test]
fn incomplete_malformed_foreign_or_oversized_inputs_refuse_before_ranking() {
    let gear = checked_text_work();
    let port = &gear.inputs[0].port_id;
    assert!(matches!(
        CostDemand::admit(&gear, &[]),
        Err(CostEvidenceRefusal::InvalidWorkInput)
    ));
    for value in [
        ValuePayload {
            value_kind: TEXT_INFO_ID.into(),
            encoded: vec![0xff],
        },
        ValuePayload {
            value_kind: COUNT_INFO_ID.into(),
            encoded: 7u64.to_le_bytes().to_vec(),
        },
    ] {
        let input = [CostInput {
            port,
            value: &value,
        }];
        assert!(matches!(
            CostDemand::admit(&gear, &input),
            Err(CostEvidenceRefusal::InvalidWorkInput)
        ));
    }
    let value = ValuePayload {
        value_kind: TEXT_INFO_ID.into(),
        encoded: vec![b'a'; MAXIMUM_COST_INPUT_BYTES + 1],
    };
    assert!(matches!(
        CostDemand::admit(
            &gear,
            &[CostInput {
                port,
                value: &value
            }]
        ),
        Err(CostEvidenceRefusal::InputLimit)
    ));
    let value = ValuePayload {
        value_kind: TEXT_INFO_ID.into(),
        encoded: vec![],
    };
    let too_many = vec![
        CostInput {
            port,
            value: &value
        };
        MAXIMUM_COST_INPUTS + 1
    ];
    assert!(matches!(
        CostDemand::admit(&gear, &too_many),
        Err(CostEvidenceRefusal::InputLimit)
    ));
    assert!(matches!(
        CostDemand::admit(
            &gear,
            &[
                CostInput {
                    port,
                    value: &value
                },
                CostInput {
                    port,
                    value: &value
                }
            ]
        ),
        Err(CostEvidenceRefusal::InvalidWorkInput)
    ));
}
