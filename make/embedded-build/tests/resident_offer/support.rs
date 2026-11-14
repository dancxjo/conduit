//! Actual checked protocol Source; CPU-only Host/resource identities.
use conduit_core as c;
use conduit_embedded_build as b;
use conduit_plot as p;
use std::collections::BTreeMap;
pub fn context() -> (
    p::CheckedSyntaxDocument,
    p::PortableExpressionProgram,
    c::PortTemporal,
) {
    context_for(c::PortTemporal::Flow { closes: true })
}
pub fn context_for(
    temporal: c::PortTemporal,
) -> (
    p::CheckedSyntaxDocument,
    p::PortableExpressionProgram,
    c::PortTemporal,
) {
    let types = p::check_syntax_document(
        &p::parse_syntax_document(include_str!(
            "../../../../plots/device-protocols/i2c-types.conduit"
        )),
        &p::StartupCatalog::new(),
    )
    .unwrap();
    let mut catalog = p::StartupCatalog::new();
    for (name, path) in [
        ("I2cTransaction", "machine/i2c/transact/request"),
        ("I2cResult", "machine/i2c/transact/result"),
    ] {
        let identity = &types
            .native_types
            .iter()
            .find(|n| n.name == name)
            .unwrap()
            .identity;
        catalog
            .insert_checked_native_type_from_document(path, &types, identity)
            .unwrap();
    }
    let source = include_str!("../../../../plots/device-protocols/ht16k33.conduit");
    let source = if temporal == c::PortTemporal::Value {
        source.replace("...|", "")
    } else {
        source.to_owned()
    };
    let doc = p::check_syntax_document(&p::parse_syntax_document(&source), &catalog).unwrap();
    let plot = p::expand_canonical_plot_for_authoring(
        &doc,
        "ht16k33-frame-request",
        &p::ProfileCatalog::new(),
    )
    .unwrap();
    let c::ConfigurationValue::Text(text) = &plot.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    let program = p::PortableExpressionProgram::from_canonical_hex(text).unwrap();
    let temporal = plot.expanded.gears[0].inputs[0].temporal;
    (doc, program, temporal)
}
pub fn bounds(private_work_units: u16) -> b::ResidentExpressionBounds {
    b::ResidentExpressionBounds {
        native: p::rust_binding::PreparedNativeBounds {
            canonical_bytes: 4096,
            scopes: 64,
            invariants: 64,
            contracts: 64,
        },
        maximum_occurrences: 4096,
        maximum_program_bytes: 512 * 1024,
        maximum_generated_bytes: 1024 * 1024,
        private_work_units,
    }
}
pub type Tables = conduit_kernel::assigned::AssignedKernelTables<1, 2, 1, 1, 1, 0>;
pub struct Assignment {
    pub advertisement: c::HostAdvertisement,
    pub model: c::Plan,
    pub encoded: Vec<u8>,
    pub tables: Tables,
    pub pins: [c::AssignedComputationRequirement; 1],
    pub maxima: c::AssignedPlanMaxima,
    pub fore: Vec<c::AssignedForeRequirement>,
}
impl Assignment {
    pub fn selected<'a>(
        &'a self,
        active: &'a c::ActivePlayIdentity,
    ) -> conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'a>
    {
        self.selected_scope(
            conduit_plan_lowering::assigned_play_scope::AssignedPlayScope::Plot(active),
        )
    }
    pub fn selected_scope<'a>(
        &'a self,
        scope: conduit_plan_lowering::assigned_play_scope::AssignedPlayScope<'a>,
    ) -> conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'a>
    {
        conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan {
            input: conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput::Contiguous(&self.encoded),
            maxima: self.maxima,
            requirements: c::AssignedPlanRequirements {
                host: c::AssignedIdentity::from_text("fixture/resident-cpu"),
                boot: c::AssignedIdentity::from_text("fixture/boot/1"),
                host_calls: &[], resources: &[0], remote_bindings: &[],
            },
            computations: c::AssignedComputationProfile {
                maximum_text_bytes: self.pins[0].text_bytes,
                maximum_total_text_bytes: self.pins[0].text_bytes,
                requirements: &self.pins,
            },
            fore: Some(&self.fore),
            fragment_id: self.model.fragments[0].fragment_id.as_str(),
            scope, node: conduit_kernel::NodeId(0), work_resource: 0,
        }
    }
    pub fn active(&self) -> c::ActivePlayIdentity {
        let f = &self.model.fragments[0];
        c::bind_active_play(&self.model.plan_id, &f.host_id, &f.boot_id, 1)
    }
}
pub fn exercise(offer: c::CapabilityOffer) -> Assignment {
    let (doc, program, temporal) = context_for(offer.inputs[0].temporal);
    let prepared =
        b::PreparedResidentExpression::prepare(&doc, &program, temporal, bounds(1000)).unwrap();
    assert_eq!(&offer.implementation.artifact_id, prepared.artifact_id());
    let plot = p::expand_canonical_plot_for_authoring(
        &doc,
        "ht16k33-frame-request",
        &p::ProfileCatalog::new(),
    )
    .unwrap();
    let bytes = program
        .maximum_prepared_input_bytes()
        .unwrap()
        .max(program.maximum_prepared_output_bytes().unwrap());
    let mut host = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/resident-cpu".into(),
        boot_id: "fixture/boot/1".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/resident-cpu".into(),
        bases: vec![],
        resources: vec![c::ResourceOffer {
            pool_id: "fixture/private-work".into(),
            class_id: c::COOPERATIVE_STEP_WORK_CLASS_ID.into(),
            capacity_units: 1000,
            compute: None,
            content: None,
        }],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let mut fore = BTreeMap::new();
    for (direction, bindings) in [
        (c::PortDirection::Input, &plot.input_bindings),
        (c::PortDirection::Output, &plot.output_bindings),
    ] {
        for binding in bindings {
            fore.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
    }
    let plan = |host: &c::HostAdvertisement| {
        let hosts = [host.clone()];
        let choices = conduit_planner::default_expanded_placements(&plot.expanded, &hosts).unwrap();
        conduit_planner::plan_expanded_authoring_with_options(
            &plot,
            &hosts,
            &choices,
            &["conduit.base/local@1".into()],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: bytes,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &fore,
        )
    };
    let selected = plan(&host).unwrap();
    let advertisement = host.clone();
    host.resources[0].capacity_units = 999;
    assert!(plan(&host).is_err());
    let fragment = &selected.fragments[0];
    assert_eq!(fragment.placements.len(), 1);
    assert_eq!(fragment.placements[0].resources[0].units, 1000);
    let bound = prepared
        .bind(
            &selected,
            &fragment.fragment_id,
            &fragment.placements[0].placement_id,
        )
        .unwrap();
    let generated = b::generate_fore_native_plan(
        &selected,
        &fragment.fragment_id,
        b::EmbeddedImageBounds::HOST_TOOLING,
        b::EmbeddedComputationBounds {
            maximum_program_bytes: 512 * 1024,
            maximum_total_program_bytes: 1024 * 1024,
        },
        2,
    )
    .unwrap();
    // CPU component envelope, not any device's runtime grant.
    let maxima = c::AssignedPlanMaxima {
        encoded_bytes: 8192,
        runtime_state_bytes: 8192,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    };
    let pin = [bound.computation_requirement()];
    let encoded = generated
        .encode_with_resident_computations(maxima, std::slice::from_ref(&bound))
        .unwrap();
    let layout = generated
        .layout_requirements_with_resident_computations(maxima, std::slice::from_ref(&bound))
        .unwrap();
    let required = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
        host_calls: &[],
        resources: &[0],
        remote_bindings: &[],
    };
    let computations = c::AssignedComputationProfile {
        maximum_text_bytes: pin[0].text_bytes,
        maximum_total_text_bytes: pin[0].text_bytes,
        requirements: &pin,
    };
    let view = c::decode_assigned_plan_with_fore(
        &encoded,
        maxima,
        required,
        computations,
        generated.fore(),
    )
    .unwrap();
    let tables =
        conduit_kernel::assigned::AssignedKernelTables::<1, 2, 1, 1, 1, 0>::prepare_with_fore(
            &encoded,
            maxima,
            required,
            computations,
            &conduit_kernel::assigned::ResidentLayout {
                records: &layout,
                counts: view.counts,
            },
            generated.fore(),
        )
        .unwrap();
    assert_eq!(
        tables.identity.plan,
        c::AssignedIdentity::from_text(selected.plan_id.as_str())
    );
    Assignment {
        advertisement,
        model: selected,
        encoded,
        tables,
        pins: pin,
        maxima,
        fore: generated.fore().to_vec(),
    }
}
