//! Checking immutable semantic metadata must not duplicate its large payload.
use conduit_core::*;
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;

fn contract(initialized: bool, text_bytes: usize) -> Kind {
    let mut kind = Kind {
        startup_parameters: vec![FrontStartupParameter {
            name: "caption".into(),
            value_type: kind_id(TEXT_INFO_ID),
            has_default: true,
        }],
        shorthand: None,
        kind_id: kind_id("test/borrowed-validation"),
        kind_contract_revision: "v1".into(),
        inputs: vec![],
        outputs: vec![],
        configuration: vec![KindConfigurationField {
            key: "caption".into(),
            default_value: ConfigurationValue::Text("x".repeat(text_bytes)),
            rule: KindConfigurationRule::TextBytes {
                maximum: u32::try_from(text_bytes).unwrap(),
            },
        }],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 1,
        },
    };
    if initialized {
        kind.inputs.push(PortDescriptor {
            port_id: port_id("update"),
            value_kind: kind_id(BOOL_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: false },
            abnormal_kind: None,
        });
        kind.outputs.push(PortDescriptor {
            port_id: port_id("current"),
            value_kind: kind_id(BOOL_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
            abnormal_kind: None,
        });
        kind.semantic_laws = vec![
            KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
            KindSemanticLaw::InitializedState(InitializedStateLaw {
                update_port_id: port_id("update"),
                current_port_id: port_id("current"),
                current: CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap(),
                initial: vec![0],
            }),
        ];
    }
    kind
}

fn check_borrowed_validation(initialized: bool) {
    let text_bytes = 64 * 1024;
    let kind = contract(initialized, text_bytes);
    let offer = BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: "test/back".into(),
            execution_profile_id: "test/native".into(),
            implementation_id: "test/implementation".into(),
            artifact_id: "test/artifact".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let (result, kind_cost) = probe::observe(|| kind.validate());
    assert_eq!(result, Ok(()));
    let (result, offer_cost) = probe::observe(|| offer.validate_constructed_semantic_contract());
    assert_eq!(result, Ok(()));
    // This catches duplication of the actual immutable text, not a grant or
    // a general promise that semantic verification makes no allocations.
    assert!(kind_cost.peak_bytes < text_bytes, "{kind_cost:?}");
    assert!(offer_cost.peak_bytes < text_bytes, "{offer_cost:?}");
}

#[test]
fn offer_validation_borrows_large_immutable_configuration() {
    check_borrowed_validation(false);
}

#[test]
fn initialized_state_validation_borrows_large_immutable_configuration() {
    check_borrowed_validation(true);
}
