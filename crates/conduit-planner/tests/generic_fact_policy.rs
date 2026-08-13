use conduit_ai::{
    ACCELERATOR_SLOT_RESOURCE, CPU_EXECUTION_RESOURCE, DATA_EGRESS_CHARACTERISTIC,
    GENERATE_TEXT_HOST_OPERATION, MAXIMUM_CONTEXT_CHARACTERISTIC, REMOTE_GENERATE_TEXT_AUTHORITY,
};
use conduit_core::{
    CharacteristicId, CharacteristicUnit, ComputePerformanceClassId, ComputeServiceGuarantee,
    ComputeTopologyGroupId,
};
use conduit_planner::{
    select_realization_with_characteristics_and_signs, HardRealizationRequirements, PlannerFactRef,
    PlannerFactValue, PlannerPredicate, PlannerPreference, RealizationDecisionDisposition,
    RealizationPolicy, RealizationPreference, RealizationRejection,
};

mod common;
use common::{generic_policy_facts as facts, quantity, resource_observations as observations};

#[test]
fn one_hard_language_reads_every_reviewed_subject_without_copying_facts() {
    let (form, hosts, advertisements) = facts();
    let cpu = conduit_core::ResourceClassId::from(CPU_EXECUTION_RESOURCE);
    let predicates = vec![
        PlannerPredicate::AtLeast {
            fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                MAXIMUM_CONTEXT_CHARACTERISTIC,
            )),
            value: quantity(24_000, CharacteristicUnit::Tokens),
        },
        PlannerPredicate::Equal {
            fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                DATA_EGRESS_CHARACTERISTIC,
            )),
            value: PlannerFactValue::Boolean(false),
        },
        PlannerPredicate::AtMost {
            fact: PlannerFactRef::ResourceUnits(cpu.clone()),
            value: quantity(2, CharacteristicUnit::Items),
        },
        PlannerPredicate::AtLeast {
            fact: PlannerFactRef::ComputeServiceGuarantee(cpu.clone()),
            value: PlannerFactValue::ServiceGuarantee(ComputeServiceGuarantee::Reserved),
        },
        PlannerPredicate::In {
            fact: PlannerFactRef::ComputePerformanceClass {
                resource_class_id: cpu.clone(),
                topology_group_id: ComputeTopologyGroupId::from("cluster-performance"),
            },
            values: vec![
                PlannerFactValue::Category("big".into()),
                PlannerFactValue::Category("performance".into()),
            ],
        },
        PlannerPredicate::NotEqual {
            fact: PlannerFactRef::ComputeNominalClockHz {
                resource_class_id: cpu.clone(),
                topology_group_id: ComputeTopologyGroupId::from("cluster-performance"),
            },
            value: quantity(1_000_000_000, CharacteristicUnit::Hertz),
        },
        PlannerPredicate::AtLeast {
            fact: PlannerFactRef::OfferQueueItems,
            value: quantity(1, CharacteristicUnit::Items),
        },
        PlannerPredicate::Equal {
            fact: PlannerFactRef::RequiresAuthority(conduit_core::AuthorityContractId::from(
                REMOTE_GENERATE_TEXT_AUTHORITY,
            )),
            value: PlannerFactValue::Boolean(false),
        },
        PlannerPredicate::Equal {
            fact: PlannerFactRef::RequiresHostOperation(
                conduit_core::HostOperationContractId::from(GENERATE_TEXT_HOST_OPERATION),
            ),
            value: PlannerFactValue::Boolean(true),
        },
        PlannerPredicate::AtLeast {
            fact: PlannerFactRef::ObservationUnreservedUnits(conduit_core::ResourceClassId::from(
                ACCELERATOR_SLOT_RESOURCE,
            )),
            value: quantity(1, CharacteristicUnit::Items),
        },
    ];
    let selection = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements {
            predicates,
            ..HardRealizationRequirements::default()
        },
        &observations(&hosts),
        &RealizationPolicy::default(),
    )
    .expect("generic predicates select the one fully admissible candidate");
    assert_eq!(selection.choice.host_id.as_str(), "ai-large-local");
    let small = selection
        .signs
        .iter()
        .find(|record| record.host_id.as_str() == "ai-small-local")
        .expect("small candidate evidence exists");
    assert_eq!(
        small.disposition,
        RealizationDecisionDisposition::Rejected(RealizationRejection::HardPredicate {
            clause_index: 0,
            fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                MAXIMUM_CONTEXT_CHARACTERISTIC,
            )),
        })
    );
    let remote = selection
        .signs
        .iter()
        .find(|record| record.host_id.as_str() == "ai-remote-base")
        .expect("remote candidate evidence exists");
    assert_eq!(
        remote.disposition,
        RealizationDecisionDisposition::Rejected(RealizationRejection::HardPredicate {
            clause_index: 1,
            fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                DATA_EGRESS_CHARACTERISTIC,
            )),
        })
    );
}

#[test]
fn generic_soft_policy_is_lexicographic_and_records_the_decisive_clause() {
    let (form, hosts, advertisements) = facts();
    let generic = RealizationPolicy {
        preferences: vec![
            RealizationPreference::Fact(PlannerPreference::PreferOrder {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    DATA_EGRESS_CHARACTERISTIC,
                )),
                values: vec![
                    PlannerFactValue::Boolean(false),
                    PlannerFactValue::Boolean(true),
                ],
            }),
            RealizationPreference::Fact(PlannerPreference::Maximize {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    MAXIMUM_CONTEXT_CHARACTERISTIC,
                )),
            }),
        ],
    };
    let selection = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        &observations(&hosts),
        &generic,
    )
    .expect("generic lexicographic policy selects large local");
    assert_eq!(selection.choice.host_id.as_str(), "ai-large-local");
    let selected = selection
        .signs
        .iter()
        .find(|record| record.disposition == RealizationDecisionDisposition::Selected)
        .expect("selected evidence exists");
    assert_eq!(selected.decisive_preference_clause, Some(1));

    let legacy = RealizationPolicy {
        preferences: vec![
            RealizationPreference::PreferCharacteristicFlag {
                characteristic_id: CharacteristicId::from(DATA_EGRESS_CHARACTERISTIC),
                value: false,
            },
            RealizationPreference::MaximizeCharacteristicCount(CharacteristicId::from(
                MAXIMUM_CONTEXT_CHARACTERISTIC,
            )),
        ],
    };
    let legacy_selection = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        &observations(&hosts),
        &legacy,
    )
    .expect("legacy policy lowers to the same ordering");
    assert_eq!(selection.choice, legacy_selection.choice);
}

#[test]
fn generic_hard_gate_cannot_be_resurrected_by_a_soft_favorite() {
    let (form, hosts, advertisements) = facts();
    let egress = PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
        DATA_EGRESS_CHARACTERISTIC,
    ));
    let selection = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements {
            predicates: vec![PlannerPredicate::Equal {
                fact: egress.clone(),
                value: PlannerFactValue::Boolean(false),
            }],
            ..HardRealizationRequirements::default()
        },
        &observations(&hosts),
        &RealizationPolicy {
            preferences: vec![RealizationPreference::Fact(
                PlannerPreference::PreferEqual {
                    fact: egress.clone(),
                    value: PlannerFactValue::Boolean(true),
                },
            )],
        },
    )
    .expect("hard-admissible local candidate wins");
    assert_ne!(selection.choice.host_id.as_str(), "ai-remote-base");
    assert!(selection.signs.iter().any(|record| {
        record.host_id.as_str() == "ai-remote-base"
            && record.disposition
                == RealizationDecisionDisposition::Rejected(RealizationRejection::HardPredicate {
                    clause_index: 0,
                    fact: egress.clone(),
                })
    }));
}

#[test]
fn missing_soft_fact_is_unknown_and_worse_instead_of_becoming_zero() {
    let (form, hosts, mut advertisements) = facts();
    let small = advertisements
        .iter_mut()
        .find(|item| item.host_id.as_str() == "ai-small-local")
        .expect("small advertisement exists");
    small.characteristics.retain(|item| {
        item.definition.characteristic_id.as_str() != MAXIMUM_CONTEXT_CHARACTERISTIC
    });
    let selection = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        &observations(&hosts),
        &RealizationPolicy {
            preferences: vec![RealizationPreference::Fact(PlannerPreference::Minimize {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    MAXIMUM_CONTEXT_CHARACTERISTIC,
                )),
            })],
        },
    )
    .expect("known quantities rank ahead of a missing value");
    assert_eq!(selection.choice.host_id.as_str(), "ai-large-local");

    let absent = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements {
            predicates: vec![PlannerPredicate::Absent {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    MAXIMUM_CONTEXT_CHARACTERISTIC,
                )),
            }],
            ..HardRealizationRequirements::default()
        },
        &observations(&hosts),
        &RealizationPolicy::default(),
    )
    .expect("explicit absence selects only the candidate missing the known optional fact");
    assert_eq!(absent.choice.host_id.as_str(), "ai-small-local");
}

#[test]
fn invalid_units_types_unknown_ids_and_clause_overflow_refuse() {
    let (form, hosts, advertisements) = facts();
    let observations = observations(&hosts);
    let invalid_requirements = [
        HardRealizationRequirements {
            predicates: vec![PlannerPredicate::AtLeast {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    DATA_EGRESS_CHARACTERISTIC,
                )),
                value: PlannerFactValue::Boolean(false),
            }],
            ..HardRealizationRequirements::default()
        },
        HardRealizationRequirements {
            predicates: vec![PlannerPredicate::AtLeast {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    MAXIMUM_CONTEXT_CHARACTERISTIC,
                )),
                value: quantity(64, CharacteristicUnit::Bytes),
            }],
            ..HardRealizationRequirements::default()
        },
        HardRealizationRequirements {
            predicates: vec![PlannerPredicate::Equal {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    "unknown/characteristic@1",
                )),
                value: PlannerFactValue::Boolean(false),
            }],
            ..HardRealizationRequirements::default()
        },
        HardRealizationRequirements {
            predicates: vec![
                PlannerPredicate::Equal {
                    fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                        DATA_EGRESS_CHARACTERISTIC,
                    )),
                    value: PlannerFactValue::Boolean(true),
                },
                PlannerPredicate::Equal {
                    fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                        DATA_EGRESS_CHARACTERISTIC,
                    )),
                    value: PlannerFactValue::Boolean(false),
                },
            ],
            ..HardRealizationRequirements::default()
        },
    ];
    for requirements in invalid_requirements {
        let error = select_realization_with_characteristics_and_signs(
            &form.gears[0],
            &hosts,
            &advertisements,
            &requirements,
            &observations,
            &RealizationPolicy::default(),
        )
        .expect_err("invalid generic hard comparison refuses");
        assert!(matches!(
            error,
            conduit_planner::PlannerError::InvalidHardRealizationRequirement(_)
        ));
    }

    let error = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        &observations,
        &RealizationPolicy {
            preferences: vec![RealizationPreference::Fact(PlannerPreference::Minimize {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    DATA_EGRESS_CHARACTERISTIC,
                )),
            })],
        },
    )
    .expect_err("boolean magnitude preference refuses");
    assert!(matches!(
        error,
        conduit_planner::PlannerError::InvalidRealizationPolicy(_)
    ));

    let error = conduit_planner::select_realization_with_policy(
        &form.gears[0],
        &hosts,
        &HardRealizationRequirements::default(),
        &RealizationPolicy {
            preferences: vec![RealizationPreference::Fact(PlannerPreference::Minimize {
                fact: PlannerFactRef::ObservationUtilizedUnits(
                    conduit_core::ResourceClassId::from(CPU_EXECUTION_RESOURCE),
                ),
            })],
        },
    )
    .expect_err("a facade without observation inputs cannot ignore an observation policy");
    assert!(matches!(
        error,
        conduit_planner::PlannerError::InvalidRealizationPolicy(_)
    ));

    let overflow = HardRealizationRequirements {
        predicates: vec![
            PlannerPredicate::AtLeast {
                fact: PlannerFactRef::OfferQueueItems,
                value: quantity(0, CharacteristicUnit::Items),
            };
            conduit_planner::MAXIMUM_PLANNER_POLICY_CLAUSES + 1
        ],
        ..HardRealizationRequirements::default()
    };
    let error = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &overflow,
        &observations,
        &RealizationPolicy::default(),
    )
    .expect_err("policy clause storage is bounded");
    assert!(matches!(
        error,
        conduit_planner::PlannerError::PlannerLimitExceeded(_)
    ));
}

#[test]
fn retained_r2_variants_lower_into_the_common_fact_vocabulary() {
    let resource = conduit_core::ResourceClassId::from(CPU_EXECUTION_RESOURCE);
    let characteristic = CharacteristicId::from(MAXIMUM_CONTEXT_CHARACTERISTIC);
    let authority = conduit_core::AuthorityContractId::from(REMOTE_GENERATE_TEXT_AUTHORITY);
    let operation = conduit_core::HostOperationContractId::from(GENERATE_TEXT_HOST_OPERATION);
    let cases = [
        RealizationPreference::MinimizeResourceUnits(resource.clone()),
        RealizationPreference::MaximizeComputeServiceGuarantee(resource.clone()),
        RealizationPreference::PreferComputePerformanceClass {
            resource_class_id: resource.clone(),
            performance_class_id: ComputePerformanceClassId::from("performance"),
        },
        RealizationPreference::MaximizeQueueItems,
        RealizationPreference::MaximizeQueueBytes,
        RealizationPreference::PreferWithoutHostOperation(operation.clone()),
        RealizationPreference::PreferWithoutAuthority(authority.clone()),
        RealizationPreference::MinimizeCharacteristicCount(characteristic.clone()),
        RealizationPreference::MaximizeCharacteristicCount(characteristic.clone()),
        RealizationPreference::PreferCharacteristicFlag {
            characteristic_id: characteristic.clone(),
            value: true,
        },
    ];
    let lowered = cases
        .iter()
        .map(|preference| preference.lower().fact().clone())
        .collect::<Vec<_>>();
    assert_eq!(
        lowered,
        vec![
            PlannerFactRef::ResourceUnits(resource.clone()),
            PlannerFactRef::ComputeServiceGuarantee(resource.clone()),
            PlannerFactRef::ComputeHasPerformanceClass {
                resource_class_id: resource,
                performance_class_id: ComputePerformanceClassId::from("performance"),
            },
            PlannerFactRef::OfferQueueItems,
            PlannerFactRef::OfferQueueBytes,
            PlannerFactRef::RequiresHostOperation(operation),
            PlannerFactRef::RequiresAuthority(authority),
            PlannerFactRef::RealizationCharacteristic(characteristic.clone()),
            PlannerFactRef::RealizationCharacteristic(characteristic.clone()),
            PlannerFactRef::RealizationCharacteristic(characteristic.clone()),
        ]
    );

    let requirements = HardRealizationRequirements {
        minimum_characteristic_counts: std::collections::BTreeMap::from([(
            characteristic.clone(),
            conduit_core::CharacteristicQuantity {
                value: 65_536,
                unit: CharacteristicUnit::Tokens,
            },
        )]),
        required_characteristic_flags: std::collections::BTreeMap::from([(
            CharacteristicId::from(DATA_EGRESS_CHARACTERISTIC),
            false,
        )]),
        ..HardRealizationRequirements::default()
    };
    assert_eq!(
        requirements.lower_characteristic_predicates(),
        vec![
            PlannerPredicate::AtLeast {
                fact: PlannerFactRef::RealizationCharacteristic(characteristic),
                value: quantity(65_536, CharacteristicUnit::Tokens),
            },
            PlannerPredicate::Equal {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    DATA_EGRESS_CHARACTERISTIC,
                )),
                value: PlannerFactValue::Boolean(false),
            },
        ]
    );
}
