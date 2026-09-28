use super::*;

pub(super) const MASK_SOURCE: &str = "form browser-graphical (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: presentation/browser-dom-mask\n face >> mask.presentation\n mask.interaction >> interaction\n mask.show >> show\n}\n";
pub(super) const ALTERNATE_MASK_SOURCE: &str = "form browser-graphical-alternate (\n >> face: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: presentation/browser-dom-mask\n face >> mask.presentation\n mask.interaction >> interaction\n mask.show >> show\n}\n";

fn port(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

pub(super) fn planned_mask(
    host_id: HostId,
    boot_id: BootId,
    source: &str,
    name: &str,
) -> Result<PlannedMaskForm, String> {
    let definition = Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id("presentation/browser-dom-mask"),
        kind_contract_revision: KindIdentity::from("conduit.browser/presentation-dom-mask@1"),
        inputs: vec![port(
            "presentation",
            PRESENTATION_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )],
        outputs: vec![
            port(
                "interaction",
                FACE_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            ),
        ],
        configuration: vec![],
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: MASK_BYTES,
        },
    };
    let mut startup = StartupCatalog::new();
    install_mask_form_value_aliases(&mut startup).map_err(|error| format!("{error:?}"))?;
    startup
        .insert(KindSignature {
            kind: definition.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|error| format!("{error:?}"))?;
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert_kind(definition.clone())
        .map_err(|error| format!("{error:?}"))?;
    let (mask, authoring) = mask_form(source, name, &startup, &profiles)?;
    let syntax = parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let offer = BackOfferBuilder::new(
        definition,
        Back {
            capability_id: CapabilityId::from("capability/browser-dom-mask"),
            execution_profile_id: ExecutionProfileId::from("browser/mask@1"),
            implementation_id: ImplementationId::from("implementation/browser-dom-mask"),
            artifact_id: ArtifactId::from("artifact/browser-runtime"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(MASK_OPERATION),
                target_kind: Some(kind_id("presentation/browser-dom-mask")),
                maximum_in_flight: 1,
                maximum_input_bytes: MASK_BYTES,
                maximum_output_bytes: MASK_BYTES,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/mask@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let placements = conduit_planner::default_expanded_placements(
        &authoring.expanded,
        core::slice::from_ref(&host),
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut boundary_limits = BTreeMap::new();
    for (direction, bindings) in [
        (PortDirection::Input, authoring.input_bindings.as_slice()),
        (PortDirection::Output, authoring.output_bindings.as_slice()),
    ] {
        for binding in bindings {
            boundary_limits.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 4,
                    byte_capacity: MASK_BYTES,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &[host],
        &placements,
        &[],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: MASK_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .map_err(|error| format!("{error:?}"))?;
    let planned = PlannedMaskForm::admit(&mask, &plan).map_err(|error| format!("{error:?}"))?;
    Ok(planned)
}

fn mask_form(
    source: &str,
    name: &str,
    startup: &StartupCatalog,
    profiles: &ProfileCatalog,
) -> Result<(MaskForm, conduit_form::ExpandedAuthoringForm), String> {
    let syntax = parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let checked = check_syntax_document(&syntax, startup).map_err(|error| format!("{error:?}"))?;
    let authoring = expand_canonical_form_for_authoring(&checked, name, profiles)
        .map_err(|error| format!("{error:?}"))?;
    let mask = MaskForm::admit(&authoring).map_err(|error| format!("{error:?}"))?;
    Ok((mask, authoring))
}

pub(super) fn admitted_routes(
    body_plan: &conduit_body::BodyPlan,
    mask: &PlannedMaskForm,
    alternate: &PlannedMaskForm,
    initial_available: bool,
    alternate_available: bool,
) -> Result<AdmittedMaskFormRoutes, String> {
    let initial_placements = mask
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| placement.placement_id.clone())
        .collect::<Vec<_>>();
    let alternate_placements = alternate
        .plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .map(|p| p.placement_id.clone())
        .collect();
    AdmittedMaskFormRoutes::new(
        body_plan,
        &[mask.clone(), alternate.clone()],
        vec![
            SealedMaskFormRoute {
                route_id: "route/browser-graphical".into(),
                mask_form: mask.mask.form_identity.clone(),
                plan_id: body_plan.plan_id.clone(),
                placement_ids: initial_placements,
                currently_available: initial_available,
            },
            SealedMaskFormRoute {
                route_id: "route/browser-graphical-fallback".into(),
                mask_form: alternate.mask.form_identity.clone(),
                plan_id: body_plan.plan_id.clone(),
                placement_ids: alternate_placements,
                currently_available: alternate_available,
            },
        ],
    )
    .map_err(|error| format!("admit browser Mask routes: {error:?}"))
}
