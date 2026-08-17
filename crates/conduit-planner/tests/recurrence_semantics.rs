use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    KindContractRevision, KindId, OfferGeneration, StructuredConfigurationValue,
    StructuredFieldType, StructuredInfoType, StructuredInfoValue, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, CanonicalStartupValue,
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};

const KIND: &str = "time/expand-recurrence";

fn civil_recurrence_type() -> StructuredInfoType {
    let text = StructuredInfoType::leaf(KindId::from("value/text@1")).unwrap();
    let count = StructuredInfoType::leaf(KindId::from("value/count@1")).unwrap();
    let local_date = StructuredInfoType::leaf(KindId::from("time/local-date@1")).unwrap();
    StructuredInfoType::record(
        KindId::from("time/civil-recurrence@1"),
        vec![
            StructuredFieldType::new("first_date", local_date.clone()).unwrap(),
            StructuredFieldType::new(
                "local_time",
                StructuredInfoType::leaf(KindId::from("time/local-time@1")).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new("zone", text.clone()).unwrap(),
            StructuredFieldType::new("rule_set", text).unwrap(),
            StructuredFieldType::new("weekdays", count.clone()).unwrap(),
            StructuredFieldType::new("maximum_occurrences", count).unwrap(),
            StructuredFieldType::new(
                "excluded_dates",
                StructuredInfoType::collection(local_date, Some(1)).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn checked_document() -> (conduit_form::CheckedSyntaxDocument, StructuredInfoValue) {
    let mut startup = StartupCatalog::new();
    startup
        .insert_structured_type("CivilRecurrence", civil_recurrence_type())
        .unwrap();
    startup
        .insert(KindSignature {
            kind: KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "schedule".into(),
                value_type: "CivilRecurrence".into(),
                default: None,
            }],
        })
        .unwrap();
    let source = r#"form meeting {
  expand: time/expand-recurrence({ first_date: "2026-03-02", local_time: "09:00:00", zone: "America/Los_Angeles", rule_set: "tzdb/2026a", weekdays: 1, maximum_occurrences: 36, excluded_dates: ["2026-03-09"] })
}
"#;
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let value = {
        let CanonicalStartupValue::Structured(value) =
            &checked.forms[0].gears[0].startup_bindings[0].value
        else {
            panic!("recurrence must become checked structured semantics")
        };
        value.try_concrete().unwrap()
    };
    (checked, value)
}

fn definition(value: &StructuredInfoValue) -> KindDefinition {
    let profile = value.value_type().profile().unwrap().value_kind().clone();
    let default_value =
        StructuredConfigurationValue::new(profile.clone(), value.canonical_bytes().unwrap())
            .unwrap();
    KindDefinition {
        kind_id: KindId::from(KIND),
        kind_contract_revision: KindContractRevision::from("time/expand-recurrence@1"),
        inputs: vec![],
        outputs: vec![],
        configuration: vec![ConfigurationField {
            key: "schedule".into(),
            default_value: ConfigurationValue::Structured(default_value),
            validation: ConfigurationRule::Structured { profile },
        }],
    }
}

fn advertisement(
    definition: &KindDefinition,
    startup_parameters: Vec<conduit_core::FaceStartupParameter>,
) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/time-test"),
        boot_id: BootId::from("boot/time-test"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/time-host"),
        resources: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters,
            shorthand: None,
            capability_id: CapabilityId::from("time-recurrence"),
            kind_id: definition.kind_id.clone(),
            kind_contract_revision: definition.kind_contract_revision.clone(),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from("test/time-recurrence-hosted@1"),
                implementation_id: ImplementationId::from("test/time-recurrence-v1"),
                artifact_id: ArtifactId::from("test/time-recurrence-artifact@1"),
            },
            inputs: vec![],
            outputs: vec![],
            host_operations: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        }],
        planner_capabilities: vec![],
    }
}

#[test]
fn recurrence_survives_checked_form_and_exact_plan_as_typed_semantics() {
    let (checked, value) = checked_document();
    let definition = definition(&value);
    let mut catalog = ProfileCatalog::new();
    catalog.insert(definition.clone()).unwrap();
    let expanded = expand_canonical_form(&checked, "meeting", &catalog).unwrap();
    let advertised = advertisement(&definition, expanded.gears[0].startup_parameters.clone());
    let placements =
        default_expanded_placements(&expanded, core::slice::from_ref(&advertised)).unwrap();
    let plan = plan_expanded_canonical(
        &expanded,
        core::slice::from_ref(&advertised),
        &placements,
        &[],
    )
    .unwrap();
    let planned = &plan.fragments[0].placements[0].configuration[0];
    let ConfigurationValue::Structured(planned_value) = &planned.value else {
        panic!("planner erased structured recurrence semantics")
    };
    assert_eq!(planned.key, "schedule");
    assert_eq!(
        planned_value.canonical_value(),
        value.canonical_bytes().unwrap()
    );
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(planned_value.canonical_value()).unwrap(),
        value
    );
}
