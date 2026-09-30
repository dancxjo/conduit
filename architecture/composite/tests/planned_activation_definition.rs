use conduit_composite::KernelCompositeDefinition;
use conduit_core::{
    prepare_plan_on_hosts, ActivePlayId, HostPreparationRefusal, Plan, PlanFragment,
    PlanPreparationHost, PreparationHostIdentity, PreparedFragmentReceipt,
};

#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

struct Host {
    identity: PreparationHostIdentity,
    receipts: Vec<PreparedFragmentReceipt>,
}

impl Host {
    fn new() -> Self {
        let fragment = common::fragment();
        Self {
            identity: PreparationHostIdentity {
                host_id: fragment.host_id,
                boot_id: fragment.boot_id,
                offer_generation: fragment.offer_generation,
            },
            receipts: vec![],
        }
    }
}

impl PlanPreparationHost for Host {
    fn preparation_identity(&self) -> PreparationHostIdentity {
        self.identity.clone()
    }
    fn prepare_fragment(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<PreparedFragmentReceipt, HostPreparationRefusal> {
        let receipt = PreparedFragmentReceipt::new(fragment);
        self.receipts.push(receipt.clone());
        Ok(receipt)
    }
    fn release_fragment(
        &mut self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        let index = self
            .receipts
            .iter()
            .position(|candidate| candidate == receipt)
            .ok_or(HostPreparationRefusal::PreparedBindingMismatch)?;
        self.receipts.remove(index);
        Ok(())
    }
    fn validate_start(
        &self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        self.receipts
            .contains(receipt)
            .then_some(())
            .ok_or(HostPreparationRefusal::PreparedBindingMismatch)
    }
    fn start_fragment(&mut self, _: &PreparedFragmentReceipt) -> ActivePlayId {
        ActivePlayId::from("play")
    }
}

fn activation_plan() -> Plan {
    let mut child_fragment = common::fragment();
    child_fragment.fore_ports = vec![
        conduit_core::PlannedForePort {
            front_port_id: conduit_core::port_id("in"),
            direction: conduit_core::PortDirection::Input,
            placement_id: conduit_core::PlacementId::from("placement"),
            gear_port_id: conduit_core::port_id("next"),
            value_kind: conduit_core::kind_id("fixture/byte@1"),
            value_contract: None,
            abnormal_kind: None,
            track: conduit_core::ConnectionTrack::Payload,
            temporal: conduit_core::PortTemporal::Value,
            pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
        },
        conduit_core::PlannedForePort {
            front_port_id: conduit_core::port_id("out"),
            direction: conduit_core::PortDirection::Output,
            placement_id: conduit_core::PlacementId::from("placement"),
            gear_port_id: conduit_core::port_id("current"),
            value_kind: conduit_core::kind_id("fixture/byte@1"),
            value_contract: None,
            abnormal_kind: None,
            track: conduit_core::ConnectionTrack::Payload,
            temporal: conduit_core::PortTemporal::Value,
            pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
        },
    ];
    let child = common::seal(child_fragment);
    let outer = common::fragment();
    conduit_core::seal_plan_with_activations(
        conduit_core::FormIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_form_id: outer.checked_form_id.clone(),
            expanded_form_id: outer.expanded_form_id.clone(),
        },
        conduit_core::PlanCompletionPolicy::Live,
        vec![],
        vec![conduit_core::PlannedActivation {
            activation_id: "each".into(),
            owner_placement_id: conduit_core::PlacementId::from("placement"),
            selected_plan_id: child.plan_id.clone(),
            selected_plan: Box::new(child),
            input: conduit_core::PlannedActivationFront {
                front_port_id: conduit_core::port_id("in"),
                value_kind: conduit_core::kind_id("fixture/byte@1"),
                abnormal_kind: None,
            },
            output: conduit_core::PlannedActivationFront {
                front_port_id: conduit_core::port_id("out"),
                value_kind: conduit_core::kind_id("fixture/byte@1"),
                abnormal_kind: None,
            },
            limits: conduit_core::PlannedActivationLimits {
                maximum_active: 1,
                maximum_queue_items: 1,
                maximum_queue_bytes: 1,
                maximum_items: 2,
            },
            terminal_policy:
                conduit_core::PlannedActivationTerminalPolicy::DrainThenPropagateExact,
            cancellation_policy:
                conduit_core::PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
            effect_multiplicity:
                conduit_core::PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
            per_activation_sign_budget: conduit_core::SignStorageBudget {
                item_capacity: 2,
                byte_capacity: 64,
            },
        }],
        vec![outer],
    )
}

#[test]
fn definition_is_derived_only_from_verified_plan_and_subordinate_receipt() {
    let plan = activation_plan();
    let mut host = Host::new();
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    let definition =
        KernelCompositeDefinition::from_planned_activation(&plan, &prepared, "each").unwrap();
    let conduit_core::PlannedActivationEntry::Unary(planned) = &plan.activations[0] else {
        unreachable!()
    };
    assert_eq!(
        definition.internal_plan.plan_id,
        planned.selected_plan.plan_id
    );
    assert_eq!(
        definition.boundary.input_fronts[0]
            .external_port
            .port_id
            .as_str(),
        "in"
    );
    assert_eq!(
        definition.boundary.output_fronts[0]
            .external_port
            .port_id
            .as_str(),
        "out"
    );
    assert!(KernelCompositeDefinition::from_planned_activation(&plan, &prepared, "other").is_err());
}
