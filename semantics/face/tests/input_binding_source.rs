//! Checked source declaration/expansion only; no physical binding admission.
mod common;
use conduit_face::{ActionInputBindingProposalInfo, GeneratedActionAffordance};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

#[test]
fn source_owned_start_remap_and_gameplay_policies_have_typed_fores() {
    let declarations = include_str!("../types.conduit");
    let action_start = declarations
        .find("type GeneratedActionAffordance =")
        .unwrap();
    let action_end = declarations[action_start..].find("\n}\n").unwrap() + action_start + 3;
    let binding_start = declarations
        .find("type ActionInputBindingProposalInfo =")
        .unwrap();
    let source = format!(
        "{}\n{}\n{}",
        &declarations[action_start..action_end],
        &declarations[binding_start..],
        include_str!("../components/start-input-binding.conduit")
    );
    let startup = StartupCatalog::new();
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    assert_eq!(checked.plots.len(), 3);
    let profiles = ProfileCatalog::new();
    for name in [
        "pregame-fire-binding",
        "pregame-enter-binding",
        "gameplay-fire-binding",
    ] {
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &profiles).unwrap();
        assert_eq!(expanded.input_bindings.len(), 1);
        assert_eq!(expanded.output_bindings.len(), 1);
        // Domain fixture for correlation, not a game execution or physical controller.
        let action_id = if name == "gameplay-fire-binding" {
            "game/shoot"
        } else {
            "game/start"
        };
        let face = conduit_face::Face::new_with_semantics(
            7,
            conduit_face::FaceBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            vec![conduit_face::FaceSubject {
                identity: "game/session".into(),
                role: conduit_face::FaceRole::Semantic(conduit_core::kind_id("game/session")),
                name: "Game session".into(),
            }],
            vec![],
            vec![],
            vec![],
            vec![conduit_face::FaceAction {
                identity: action_id.into(),
                intent: action_id.into(),
                target: "game/session".into(),
                name: "Activate".into(),
                arguments: vec![],
                disclosure: conduit_face::FaceDisclosureLevel::Primary,
                availability: conduit_face::FaceActionAvailability::Available,
            }],
            vec![],
        )
        .unwrap();
        let show = common::available_mask_show(&face);
        let context = ActionInputBindingProposalInfo::new(
            true,
            "boot/current".into(),
            "control/caller-forged".into(),
            "Caller label".into(),
            face.identity.as_str().into(),
            "host/current".into(),
            "plan/current-input".into(),
            2,
            GeneratedActionAffordance::new("game/caller-forged".into(), 7).unwrap(),
            show.show_id.as_str().into(),
            1,
        )
        .unwrap();
        let gear = &expanded.expanded.gears[0];
        let program = gear
            .configuration
            .iter()
            .find(|field| field.key == "program")
            .unwrap();
        let conduit_core::ConfigurationValue::Text(program) = &program.value else {
            panic!("checked program");
        };
        let decoded = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
        let host = conduit_core::HostAdvertisement {
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id: conduit_core::HostId::from("host/current"),
            boot_id: conduit_core::BootId::from("boot/current"),
            offer_generation: conduit_core::OfferGeneration(1),
            profile: conduit_core::HostProfileId::from("fixture/std-input-policy"),
            bases: vec![],
            resources: vec![],
            planner_capabilities: vec![],
            capabilities: vec![conduit_std_offers::pure_expression_std_offer(
                &decoded,
                gear.inputs[0].temporal,
            )
            .unwrap()],
        };
        let mut unavailable_host = host.clone();
        unavailable_host.capabilities.clear();
        assert!(conduit_planner::default_expanded_placements(
            &expanded.expanded,
            std::slice::from_ref(&unavailable_host)
        )
        .is_err());
        let placements = conduit_planner::default_expanded_placements(
            &expanded.expanded,
            std::slice::from_ref(&host),
        )
        .unwrap();
        let empty = std::collections::BTreeMap::new();
        let lines = std::collections::BTreeMap::new();
        let options = || conduit_planner::PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        };
        let boundaries = expanded
            .input_bindings
            .iter()
            .map(|binding| (conduit_core::PortDirection::Input, binding))
            .chain(
                expanded
                    .output_bindings
                    .iter()
                    .map(|binding| (conduit_core::PortDirection::Output, binding)),
            )
            .map(|(direction, binding)| {
                (
                    conduit_planner::ForeBoundaryKey {
                        direction,
                        front_port_id: binding.front_port_id.clone(),
                        track: binding.track,
                    },
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: 4096,
                    },
                )
            })
            .collect();
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            &expanded,
            std::slice::from_ref(&host),
            &placements,
            &[],
            options(),
            &boundaries,
        )
        .unwrap();
        let custody = conduit_face::prepare_checked_source_plan(
            &checked,
            &expanded,
            &profiles,
            &conduit_plot::CanonicalBackCatalog::new(),
            &plan,
            conduit_face::CheckedSourcePlanInputs {
                hosts: std::slice::from_ref(&host),
                placements: &placements,
                bases: &[],
                options: options(),
                boundary_limits: &boundaries,
            },
        )
        .unwrap();
        assert_eq!(custody.plan().plan_id, plan.plan_id);
        let produced =
            conduit_face::produce_checked_input_binding_proposal(&custody, context.clone())
                .unwrap();
        assert_eq!(produced.producer_plan_id(), &plan.plan_id);
        let correlated =
            conduit_face::correlate_input_binding_proposal(&produced, &face, &show).unwrap();
        assert_eq!(correlated.action().identity, action_id);
        assert_eq!(correlated.action().target, "game/session");
        // Correlation must reject independently stale identities and revisions.
        for (field, value, expected) in [
            (
                "face_identity",
                serde_json::json!("face/stale"),
                conduit_face::InputBindingCorrelationRefusal::StaleFace,
            ),
            (
                "show_identity",
                serde_json::json!("show/stale"),
                conduit_face::InputBindingCorrelationRefusal::StaleShow,
            ),
        ] {
            let mut changed = serde_json::to_value(&context).unwrap();
            changed[field] = value;
            let changed = serde_json::from_value(changed).unwrap();
            let stale =
                conduit_face::produce_checked_input_binding_proposal(&custody, changed).unwrap();
            assert!(
                matches!(conduit_face::correlate_input_binding_proposal(&stale, &face, &show), Err(actual) if actual == expected)
            );
        }
        let mut changed = serde_json::to_value(&context).unwrap();
        changed["offered_action"]["source_face_revision"] = serde_json::json!(6);
        let stale = conduit_face::produce_checked_input_binding_proposal(
            &custody,
            serde_json::from_value(changed).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            conduit_face::correlate_input_binding_proposal(&stale, &face, &show),
            Err(conduit_face::InputBindingCorrelationRefusal::StaleFace)
        ));

        for lifecycle in [
            conduit_face::ManifestationLifecycle::Closed,
            conduit_face::ManifestationLifecycle::Replaced,
            conduit_face::ManifestationLifecycle::Failed,
        ] {
            let sign = conduit_core::SignId::from("sign/retired-input-fixture");
            let retired = if lifecycle == conduit_face::ManifestationLifecycle::Failed {
                show.fail(conduit_face::ManifestationFailure::AdapterUnavailable, sign)
            } else {
                show.transition(lifecycle, sign)
            }
            .unwrap();
            assert!(matches!(
                conduit_face::correlate_input_binding_proposal(&produced, &face, &retired),
                Err(conduit_face::InputBindingCorrelationRefusal::Show)
            ));
        }
        // A fresh Face/Show does not rescue an absent or unavailable action.
        for unavailable in [false, true] {
            let actions = if unavailable {
                let mut action = face.actions[0].clone();
                action.availability = conduit_face::FaceActionAvailability::Unavailable {
                    reason_code: "game/not-ready".into(),
                    explanation: "Session unavailable".into(),
                };
                vec![action]
            } else {
                vec![]
            };
            let changed_face = conduit_face::Face::new_with_semantics(
                face.revision,
                face.basis.clone(),
                face.subjects.clone(),
                face.relationships.clone(),
                face.properties.clone(),
                face.text.clone(),
                actions,
                face.disclosures.clone(),
            )
            .unwrap();
            let changed_show = common::available_mask_show(&changed_face);
            let mut changed = serde_json::to_value(&context).unwrap();
            changed["face_identity"] = serde_json::json!(changed_face.identity.as_str());
            changed["show_identity"] = serde_json::json!(changed_show.show_id.as_str());
            let proposal = conduit_face::produce_checked_input_binding_proposal(
                &custody,
                serde_json::from_value(changed).unwrap(),
            )
            .unwrap();
            assert!(matches!(
                conduit_face::correlate_input_binding_proposal(
                    &proposal,
                    &changed_face,
                    &changed_show
                ),
                Err(conduit_face::InputBindingCorrelationRefusal::Action)
            ));
        }

        assert_eq!(
            produced.producer_plot().checked_plot_id,
            expanded.expanded.checked_plot_id
        );
        assert_eq!(
            produced.producer_plot().expanded_plot_id,
            expanded.expanded.expanded_plot_id
        );
        let mut forged = serde_json::to_value(context.clone()).unwrap();
        forged["mapping_revision"] = serde_json::json!(0);
        let forged = serde_json::from_value(forged).unwrap();
        assert!(matches!(
            conduit_face::produce_checked_input_binding_proposal(&custody, forged),
            Err(conduit_face::InputBindingProducerRefusal::Context)
        ));

        assert_eq!(plan.fragments[0].placements[0].host_id, host.host_id);
        let input = context
            .clone()
            .into_structured()
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let output = conduit_plot::PortableExpressionProgram::from_canonical_hex(program)
            .unwrap()
            .evaluate(&input)
            .unwrap();
        let output = conduit_core::StructuredInfoValue::from_canonical_bytes(&output).unwrap();
        let proposal = ActionInputBindingProposalInfo::from_structured(output).unwrap();
        let (control, label, action) = match name {
            "pregame-fire-binding" => ("control/fire", "Fire", "game/start"),
            "pregame-enter-binding" => ("keyboard/enter", "Enter", "game/start"),
            _ => ("control/fire", "Fire", "game/shoot"),
        };
        assert_eq!(produced.proposal(), &proposal);
        assert_eq!(proposal.control_identity(), control);
        assert_eq!(proposal.control_label(), label);
        assert_eq!(proposal.offered_action().action_identity(), action);
        assert_eq!(proposal.face_identity(), context.face_identity());
        assert_eq!(proposal.show_identity(), context.show_identity());
        assert_eq!(proposal.mapping_revision(), context.mapping_revision());
    }
}

#[test]
fn imported_shape_without_native_laws_cannot_construct_binding_proposals() {
    let mut startup = StartupCatalog::new();
    startup
        .insert_structured_type(
            "ActionInputBindingProposalInfo",
            ActionInputBindingProposalInfo::semantic_type().unwrap(),
        )
        .unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../components/start-input-binding.conduit")),
        &startup,
    )
    .unwrap();
    let refused = expand_canonical_plot_for_authoring(
        &checked,
        "pregame-fire-binding",
        &ProfileCatalog::new(),
    )
    .unwrap_err();
    assert_eq!(refused.code, "CND-FRM-046");
}
