//! Exact, typed planning profile for the std-host multi-value kernel gate.
//!
//! This is deliberately not the legacy `conduit.std` catalog: the filter has
//! concrete even-tick semantics and every port carries `value/tick@1`.

use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_offer, resource_requirement,
    wait_host_operation_requirement, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, HostAdvertisement, HostId, HostProfileId,
    KindContractRevision, OfferGeneration, PortDescriptor, PortDirection,
    PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION, TIMER_RESOURCE_CLASS,
};
use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};

pub const TICK_KIND: &str = "time/tick";
pub const TEE_KIND: &str = "flow/tee";
pub const EVEN_FILTER_KIND: &str = "flow/filter-even";
pub const LATEST_KIND: &str = "state/latest";
pub const SHOW_KIND: &str = "presentation/show";
pub const TICK_VALUE_KIND: &str = "value/tick@1";

const IN_PORT: &str = "in";
const OUT_PORT: &str = "out";
const TICK_PORT: &str = "tick";
const LEFT_PORT: &str = "left";
const RIGHT_PORT: &str = "right";

pub fn profile_catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    for definition in [
        definition(
            TICK_KIND,
            vec![],
            vec![port(TICK_PORT, PortDirection::Output)],
            vec![
                ConfigurationField {
                    key: "count".to_string(),
                    default_value: ConfigurationValue::U64(4),
                    validation: ConfigurationRule::U64Range {
                        minimum: 1,
                        maximum: 4,
                    },
                },
                ConfigurationField {
                    key: "period-ms".to_string(),
                    default_value: ConfigurationValue::U64(0),
                    validation: ConfigurationRule::U64Range {
                        minimum: 0,
                        maximum: u64::MAX,
                    },
                },
            ],
        ),
        definition(
            TEE_KIND,
            vec![port(IN_PORT, PortDirection::Input)],
            vec![
                port(LEFT_PORT, PortDirection::Output),
                port(RIGHT_PORT, PortDirection::Output),
            ],
            vec![],
        ),
        definition(
            EVEN_FILTER_KIND,
            vec![port(IN_PORT, PortDirection::Input)],
            vec![port(OUT_PORT, PortDirection::Output)],
            vec![],
        ),
        definition(
            LATEST_KIND,
            vec![port(IN_PORT, PortDirection::Input)],
            vec![port(OUT_PORT, PortDirection::Output)],
            vec![],
        ),
        definition(
            SHOW_KIND,
            vec![port(IN_PORT, PortDirection::Input)],
            vec![],
            vec![],
        ),
    ] {
        catalog
            .insert(definition)
            .expect("multi-value profile kinds are unique");
    }
    catalog
}

pub fn advertisement(
    host_id: HostId,
    boot_id: conduit_core::BootId,
    offer_generation: OfferGeneration,
) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation,
        profile: HostProfileId::from("conduit.std/kernel-multivalue@1"),
        resources: vec![
            resource_offer("std/kernel-presentation", PRESENTATION_RESOURCE_CLASS, 2),
            resource_offer("std/kernel-timer", TIMER_RESOURCE_CLASS, 1),
        ],
        capabilities: vec![
            offer(TICK_KIND, "time-tick", 1),
            offer(TEE_KIND, "flow-tee", 0),
            offer(EVEN_FILTER_KIND, "flow-filter-even", 0),
            offer(LATEST_KIND, "state-latest", 0),
            offer(SHOW_KIND, "presentation-show", 2),
        ],
    }
}

fn definition(
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    configuration: Vec<ConfigurationField>,
) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: revision(kind),
        inputs,
        outputs,
        configuration,
    }
}

fn offer(kind: &str, capability: &str, resource_units: u32) -> CapabilityOffer {
    let definition = profile_catalog()
        .get(&kind_id(kind))
        .expect("offered multi-value kind exists")
        .clone();
    let host_operations = match kind {
        TICK_KIND => vec![wait_host_operation_requirement()],
        SHOW_KIND => vec![present_host_operation_requirement(
            kind_id("presentation/stdout-tick@1"),
            8,
        )],
        _ => vec![],
    };
    let resource_requirements = match kind {
        TICK_KIND => vec![resource_requirement(TIMER_RESOURCE_CLASS, resource_units)],
        SHOW_KIND => vec![resource_requirement(
            PRESENTATION_RESOURCE_CLASS,
            resource_units / 2,
        )],
        _ => vec![],
    };
    CapabilityOffer {
        capability_id: CapabilityId::from(capability),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        execution_profile_id: ExecutionProfileId::from(format!(
            "conduit.std/{capability}-kernel-hosted@1"
        )),
        implementation_id: conduit_core::ImplementationId::from(format!(
            "std/kernel-{capability}@1"
        )),
        artifact_id: ArtifactId::from(format!("conduit-std-host/{capability}@1")),
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations,
        resource_requirements,
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: if kind == SHOW_KIND { 2 } else { 1 },
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(TICK_VALUE_KIND),
        direction,
    }
}

fn revision(kind: &str) -> KindContractRevision {
    KindContractRevision::from(format!("conduit.std/{kind}-tick@1"))
}

#[cfg(test)]
mod tests {
    use super::{advertisement, profile_catalog};
    use conduit_core::{BootId, ConnectionProvider, HostId, OfferGeneration};
    use conduit_form::parse;
    use conduit_planner::{default_placements, plan};
    use conduit_runtime::lowering::lower_plan_fragment;

    #[test]
    fn exact_multi_value_form_plans_and_lowers_all_numeric_tables() {
        let form = parse(
            include_str!("../../../examples/kernel-multivalue.form"),
            &profile_catalog(),
        )
        .expect("typed multi-value form parses");
        let host = advertisement(
            HostId::from("std-kernel-multivalue"),
            BootId::from("std-kernel-multivalue-boot"),
            OfferGeneration(1),
        );
        let placements = default_placements(&form, core::slice::from_ref(&host))
            .expect("one exact capability exists per operation");
        let plan = plan(
            &form,
            core::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("typed multi-value form plans");
        let lowered = lower_plan_fragment(&plan.fragments[0]).expect("fragment lowers");
        assert_eq!(lowered.nodes.len(), 6);
        assert_eq!(lowered.cords.len(), 5);
        assert_eq!(lowered.routes.len(), 5);
        assert_eq!(lowered.host_operations.len(), 3);
        assert_eq!(lowered.resources.len(), 3);
        assert_eq!(lowered.cord_value_slots, 20);
        assert_eq!(lowered.cord_value_bytes, 320);
    }
}
