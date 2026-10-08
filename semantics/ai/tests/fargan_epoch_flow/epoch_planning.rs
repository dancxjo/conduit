use super::*;

pub(super) fn numeric_epoch_offer(kind: &str, capacity64: bool) -> CapabilityOffer {
    use conduit_ai::operation_owners::*;
    (if capacity64 {
        fixed_numeric::fixed_numeric_offer_capacity64(kind)
    } else {
        fixed_numeric::fixed_numeric_offer(kind)
    })
    .or_else(|_| fixed_numeric_flow::fixed_affine_flow_offer(kind))
    .or_else(|_| fixed_numeric_linear_flow::fixed_linear_flow_offer(kind))
    .or_else(|_| fixed_numeric_embedding_flow::fixed_embedding_flow_offer(kind))
    .or_else(|_| conduit_ai::fixed_numeric_pair_flow::fixed_flow_pair_offer(kind))
    .or_else(|_| {
        if kind == "numeric/flow-u64-to-u16" {
            conduit_ai::fixed_numeric_integer_narrowing::checked_integer_narrowing_offer(true)
        } else {
            Err("different integer operation".into())
        }
    })
    .or_else(|_| {
        if kind == "numeric/flow-f32-to-i16-nearest-away160" {
            conduit_ai::fixed_numeric_float_integer::float_integer_offer(true)
        } else {
            Err(format!("unsupported exact numeric epoch owner {kind}"))
        }
    })
    .unwrap_or_else(|e| panic!("{kind}: {e}"))
}

pub(super) fn prepared_epoch_plan(
    capacity64: bool,
) -> Result<(Plan, EpochProfiles), conduit_planner::PlannerError> {
    prepare_authored_epoch_entry(
        prepared_epoch_profiles_with_capacity(capacity64),
        epoch_source(),
        "speech/flow-fargan-float-epoch",
        capacity64,
        vec![],
    )
}
pub(super) fn prepare_authored_epoch_entry(
    mut context: EpochProfiles,
    source: String,
    entry_name: &str,
    capacity64: bool,
    extra_offers: Vec<CapabilityOffer>,
) -> Result<(Plan, EpochProfiles), conduit_planner::PlannerError> {
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let mut types: std::collections::BTreeMap<_, _> = fixed_numeric_types()
        .unwrap()
        .into_iter()
        .map(|ty| (ty.name, ty.value_type))
        .collect();
    types.extend(
        checked
            .native_types
            .iter()
            .map(|ty| (ty.name.clone(), ty.value_type.clone())),
    );
    let entry = checked.plots.iter().find(|p| p.name == entry_name).unwrap();
    let mut offers: Vec<_> = context
        .native
        .iter()
        .map(|p| p.offer(true).unwrap())
        .collect();
    offers.extend(context.weakening.iter().map(|p| p.offer(true).unwrap()));
    offers.extend(context.guards.iter().map(|p| p.offer().unwrap()));
    offers.extend(extra_offers);
    offers.extend(context.zip.offers().cloned());
    offers.extend(context.pairs.iter().map(|profile| profile.offer().unwrap()));
    let period = conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition(
        "type FarganPeriod = U16 in 32..=255\n",
    )
    .unwrap();
    offers.push(period.offer(true).unwrap());
    let mut wrapper = format!("\nplot epoch-runtime-proof {{\n inner: {entry_name}\n");
    for port in &entry.runtime_ports {
        let name = format!("epoch-proof/{}", port.name.text);
        let input = port.direction == conduit_plot::syntax::RuntimePortDirection::Input;
        let ty = types.get(&port.value_type.text).unwrap_or_else(|| {
            let descriptor = entry
                .runtime_front
                .inputs()
                .iter()
                .chain(entry.runtime_front.outputs())
                .find(|descriptor| descriptor.port_id == port_id(&port.name.text))
                .expect("exact checked entry port");
            checked
                .structured_type(&descriptor.value_kind)
                .expect("exact checked alias Type")
        });
        let descriptor = PortDescriptor {
            port_id: port_id("value"),
            value_kind: ty.profile().unwrap().value_kind().clone(),
            direction: if input {
                PortDirection::Output
            } else {
                PortDirection::Input
            },
            temporal: match port.temporal {
                conduit_plot::syntax::RuntimePortTemporal::Value => PortTemporal::Value,
                conduit_plot::syntax::RuntimePortTemporal::Flow { closes } => {
                    PortTemporal::Flow { closes }
                }
                _ => panic!("epoch proof supports exact Value and Flow runtime ports"),
            },
            abnormal_kind: None,
        };
        let kind = Kind {
            kind_id: kind_id(&name),
            kind_contract_revision: name.clone().into(),
            startup_parameters: vec![],
            shorthand: None,
            configuration: vec![],
            inputs: if input {
                vec![]
            } else {
                vec![descriptor.clone()]
            },
            outputs: if input {
                vec![descriptor.clone()]
            } else {
                vec![]
            },
            semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: if input {
                    FrontValueLocation::Output(descriptor.port_id)
                } else {
                    FrontValueLocation::Input(descriptor.port_id)
                },
                contract: shape_contract(ty),
            }])],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: maximum_prepared_transport_value_bytes(ty).unwrap(),
            },
        };
        context
            .startup
            .insert(KindSignature {
                kind: name.clone(),
                startup_parameters: vec![],
            })
            .unwrap();
        context.profiles.insert_kind(kind.clone()).unwrap();
        offers.push(
            BackOfferBuilder::new(
                kind,
                Back {
                    capability_id: name.clone().into(),
                    execution_profile_id: "synthetic-epoch-driver@1".into(),
                    implementation_id: name.clone().into(),
                    artifact_id: name.clone().into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build(),
        );
        wrapper.push_str(&format!(" {}: {name}\n", port.name.text));
        wrapper.push_str(&if input {
            format!(" {}.value >> inner.{}\n", port.name.text, port.name.text)
        } else {
            format!(" inner.{} >> {}.value\n", port.name.text, port.name.text)
        });
    }
    wrapper.push_str("}\n");
    let exact_source = format!("{source}{wrapper}");
    let checked =
        check_syntax_document(&parse_syntax_document(&exact_source), &context.startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "epoch-runtime-proof", &context.profiles)
            .unwrap();
    assert!(expanded.input_bindings.is_empty() && expanded.output_bindings.is_empty());
    for gear in &expanded.expanded.gears {
        let selected = if gear.kind_id == kind_id(&period.kind_identity(true)) {
            None // Exact Source-retained profile offer was installed above.
        } else if gear.kind_id.as_str().starts_with("numeric/") {
            Some(numeric_epoch_offer(gear.kind_id.as_str(), capacity64))
        } else if matches!(
            gear.kind_contract_revision.as_str(),
            PURE_EXPRESSION_REVISION | PURE_FILTER_REVISION
        ) {
            let [entry] = gear.configuration.as_slice() else {
                panic!("exact pure Source configuration")
            };
            let ConfigurationValue::Text(encoded) = &entry.value else {
                panic!("pure source program")
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            Some(
                if gear.kind_contract_revision.as_str() == PURE_FILTER_REVISION {
                    conduit_std_host::pure_filter::offer(&program, gear.inputs[0].temporal).unwrap()
                } else {
                    conduitos::expression_host_call::offer(&program, gear.outputs[0].temporal)
                        .unwrap()
                },
            )
        } else {
            None
        };
        if let Some(selected) = selected {
            if !offers
                .iter()
                .any(|offer| offer.capability_id == selected.capability_id)
            {
                offers.push(selected);
            }
        }
    }
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "epoch-runtime-proof".into(),
        boot_id: "epoch-proof-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "synthetic-floating-epoch@1".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    };
    let placements = conduit_planner::default_expanded_placements(
        &expanded.expanded,
        std::slice::from_ref(&host),
    )?;
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded.expanded,
        &[host],
        &placements,
        &["conduit.base/local@1".into()],
    )?;
    assert!(verify_plan(&plan));
    assert_eq!(plan.source_document_id, checked.source_document_id);
    context.checked_source = Some(super::plan_artifact::CheckedEpochSource {
        text: exact_source,
        identity: checked.source_document_id.clone(),
    });
    conduit_ai::operation_owners::native_profile::NativeProfileOperationFactory::for_plan(
        &plan,
        &context.native,
    )
    .unwrap();
    conduit_ai::operation_owners::nominal_weakening::NominalWeakeningOperationFactory::for_plan(
        &plan,
        &context.weakening,
    )
    .unwrap();
    conduit_ai::operation_owners::fixed_numeric_guard::FixedGuardOperationFactory::for_plan(
        &plan,
        context.guards.clone(),
    )
    .unwrap();
    eprintln!(
        "ordinary epoch Plan: {} placements, {} cords",
        plan.fragments[0].placements.len(),
        expanded.expanded.connections.len()
    );
    Ok((plan, context))
}
