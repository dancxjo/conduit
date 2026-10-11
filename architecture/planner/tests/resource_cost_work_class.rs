//! Checked semantic work classes cannot be chosen by a favorable arbitrary ID.
mod common;
use conduit_core::ConfigurationValue;
use conduit_planner::resource_cost::*;

#[test]
fn equivalent_instances_share_class_but_checked_meaning_changes_refuse() {
    let gear = common::pulse_gear();
    let class = CostWorkClass::from_checked_gear(&gear);
    let mut replica = gear.clone();
    replica.gear_id = "pulse/replica".into();
    assert_eq!(class.validate_demand(&replica), Ok(()));
    assert_eq!(class.gear(), &gear);
    for mutation in 0..10 {
        let mut changed = replica.clone();
        match mutation {
            0 => changed.kind_id = "work/different-kind".into(),
            1 => changed.kind_contract_revision = "work/revised-law".into(),
            2 => {
                changed
                    .configuration
                    .iter_mut()
                    .find(|entry| entry.key == "count")
                    .unwrap()
                    .value = ConfigurationValue::U64(200)
            }
            3 => changed.startup_parameters.clear(),
            4 => {
                let mut input = changed.outputs[0].clone();
                input.direction = conduit_core::PortDirection::Input;
                changed.inputs.push(input);
            }
            5 => changed.outputs.clear(),
            6 => changed.semantic_contract.configuration.clear(),
            7 => changed.pool_references.push("work/other-pool".into()),
            8 => changed.shorthand = Some(("different/input".into(), "different/output".into())),
            _ => changed.outputs[0].port_id = "different/port".into(),
        }
        assert_ne!(
            changed, replica,
            "mutation {mutation} must actually change meaning"
        );
        assert_eq!(
            class.validate_demand(&changed),
            Err(CostEvidenceRefusal::WorkClassChanged),
            "mutation {mutation}"
        );
    }
}
