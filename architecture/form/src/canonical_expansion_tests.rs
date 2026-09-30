use crate::prelude::*;

use crate::{
    check_syntax_document, expand_canonical_form, expand_canonical_form_for_authoring,
    parse_syntax_document, ConfigurationValue, ExpandedCanonicalForm, KindConfigurationField,
    KindConfigurationRule, KindProjection, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use conduit_core::{
    kind_id, port_id, AbnormalTerminalTransduction, CancellationTransduction, CapabilityLimits,
    ExternalEffectBehavior, FrontStartupParameter, Kind, KindIdentity, KindSemanticLaw,
    NormalCloseTransduction, PortDescriptor, PortDirection, Quantity, QuantityUnit,
    SuspensionBehavior, TemporalStateBehavior, TerminalTransductionProfile,
};
use conduit_kernel::scheduler::{
    AssignedAbnormalTransduction, AssignedCancellationTransduction, AssignedConnectionTrack,
    AssignedNormalCloseTransduction, AssignedTerminalTransduction, CordCapacity, CordSpec,
    FixedScheduler, NodeSpec, SchedulerError, SchedulerStatus, StepBack, StepInputBytes, StepIo,
    StepOutcome,
};
use conduit_kernel::{
    CanonicalValue, CordEndpoint, CordId, FixedRoutes, FixedSignLog, FixedValueStore, NodeId,
    PortId as KernelPortId, RouteRange, RouteTarget,
};

fn canonical_kind(projection: KindProjection) -> Kind {
    let startup_parameters = projection
        .configuration
        .iter()
        .map(|field| FrontStartupParameter {
            name: field.key.clone(),
            value_type: field.default_value.semantic_kind(),
            has_default: true,
        })
        .collect();
    let shorthand = match (projection.inputs.as_slice(), projection.outputs.as_slice()) {
        ([input], [output]) => Some((input.port_id.clone(), output.port_id.clone())),
        _ => None,
    };
    Kind {
        startup_parameters,
        shorthand,
        kind_id: projection.kind_id,
        kind_contract_revision: projection.kind_contract_revision,
        inputs: projection.inputs,
        outputs: projection.outputs,
        configuration: projection.configuration,
        semantic_laws: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 1,
        },
    }
}

#[test]
fn profile_startup_catalog_preserves_quantity_dimensions() {
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/range"),
            kind_contract_revision: KindIdentity::from("test/range@1"),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![KindConfigurationField {
                key: "distance".into(),
                default_value: ConfigurationValue::Quantity(Quantity::new(
                    500,
                    QuantityUnit::Millimeter,
                )),
                rule: KindConfigurationRule::QuantityRange {
                    minimum: 0,
                    maximum: 10_000,
                    canonical_unit: QuantityUnit::Millimeter,
                },
            }],
        })
        .unwrap();

    let startup = profile.startup_catalog().unwrap();
    assert_eq!(
        startup.signature("test/range").unwrap().startup_parameters[0].value_type,
        conduit_core::DISTANCE_INFO_ID
    );
    assert_eq!(
        startup.fore("test/range").unwrap().startup_parameters()[0]
            .value_type
            .as_str(),
        conduit_core::DISTANCE_INFO_ID
    );
}

#[test]
fn canonical_kind_fore_owns_callable_default_truth() {
    let projection = KindProjection {
        kind_id: kind_id("test/required-prefix"),
        kind_contract_revision: KindIdentity::from("test/required-prefix@1"),
        inputs: vec![],
        outputs: vec![],
        configuration: vec![KindConfigurationField {
            key: "prefix".into(),
            default_value: ConfigurationValue::Text("provider-default".into()),
            rule: KindConfigurationRule::TextBytes { maximum: 64 },
        }],
    };
    let mut kind = canonical_kind(projection);
    kind.startup_parameters[0].has_default = false;
    let mut profile = ProfileCatalog::new();
    profile.insert_kind(kind).unwrap();

    let startup = profile.startup_catalog().unwrap();
    let signature = startup.signature("test/required-prefix").unwrap();
    assert_eq!(signature.startup_parameters[0].default, None);
    assert!(
        !startup
            .fore("test/required-prefix")
            .unwrap()
            .startup_parameters()[0]
            .has_default
    );
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id("test/value"),
        direction,
        temporal: conduit_core::PortTemporal::Value,
        abnormal_kind: None,
    }
}

#[test]
fn fixed_arity_relational_glyph_expands_to_one_ordinary_gear() {
    let mut profile = ProfileCatalog::new();
    profile
        .insert_kind(canonical_kind(KindProjection {
            kind_id: kind_id("test/zip"),
            kind_contract_revision: KindIdentity::from("test/zip@1"),
            inputs: vec![
                port("left", PortDirection::Input),
                port("right", PortDirection::Input),
            ],
            outputs: vec![port("paired", PortDirection::Output)],
            configuration: vec![],
        }))
        .unwrap();
    let startup = profile.startup_catalog().unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(
            "sans glyphs\nwith test/zip as &>\nform main (\n >> a: test/value\n >> b: test/value\n paired: test/value >>\n) {\n a &> b >> paired\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let authoring = expand_canonical_form_for_authoring(&checked, "main", &profile).unwrap();
    let source_expansion = &checked.source_sugar_expansions[0];
    assert_eq!(authoring.expanded.gears.len(), 1);
    assert_eq!(authoring.expanded.gears[0].kind_id.as_str(), "test/zip");
    assert_eq!(
        authoring.expanded.gears[0].kind_id.as_str(),
        source_expansion.ordinary_kind
    );
    assert_eq!(authoring.input_bindings.len(), 2);
    assert_eq!(authoring.output_bindings.len(), 1);
}

#[test]
fn variadic_relational_glyph_expands_to_one_finitely_specialized_gear() {
    let mut profile = ProfileCatalog::new();
    let mut input = port("operand", PortDirection::Input);
    input.temporal = conduit_core::PortTemporal::Flow { closes: true };
    let mut output = port("merged", PortDirection::Output);
    output.temporal = conduit_core::PortTemporal::Flow { closes: true };
    let mut merge = canonical_kind(KindProjection {
        kind_id: kind_id("flow/merge"),
        kind_contract_revision: KindIdentity::from("flow/merge@1"),
        inputs: vec![input],
        outputs: vec![output],
        configuration: vec![],
    });
    merge
        .semantic_laws
        .push(KindSemanticLaw::TerminalTransduction(
            TerminalTransductionProfile {
                input_port_id: port_id("operand"),
                output_port_id: port_id("merged"),
                normal_close: NormalCloseTransduction::PropagateAfterDrain,
                abnormal: AbnormalTerminalTransduction::NotAccepted,
                cancellation: CancellationTransduction::NotCancellable,
            },
        ));
    profile
        .insert_homogeneous_variadic_kind(merge, 2, 16)
        .unwrap();
    let startup = profile.startup_catalog().unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(
            "form main (\n >> a: test/value...|\n >> b: test/value...|\n >> c: test/value...|\n merged: test/value...| >>\n) {\n a >< b >< c >> merged\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let authoring = expand_canonical_form_for_authoring(&checked, "main", &profile).unwrap();
    let source_expansion = &checked.source_sugar_expansions[0];
    assert_eq!(authoring.expanded.gears.len(), 1);
    let gear = &authoring.expanded.gears[0];
    assert_eq!(gear.kind_id.as_str(), "flow/merge");
    assert_eq!(gear.kind_id.as_str(), source_expansion.ordinary_kind);
    assert_eq!(
        gear.kind_contract_revision.as_str(),
        "flow/merge@1/inputs/3"
    );
    assert_eq!(gear.inputs.len(), 3);
    assert_eq!(gear.terminal_transductions.len(), 3);
    assert_eq!(authoring.input_bindings.len(), 3);
    assert_eq!(authoring.output_bindings.len(), 1);
}

#[test]
fn canonical_terminal_transduction_survives_expansion_and_changes_identity() {
    fn expand(
        abnormal: AbnormalTerminalTransduction,
    ) -> (ExpandedCanonicalForm, conduit_core::CheckedFormId) {
        let mut kind = canonical_kind(KindProjection {
            kind_id: kind_id("test/terminal-transform"),
            kind_contract_revision: KindIdentity::from("test/terminal-transform@1"),
            inputs: vec![port("input", PortDirection::Input)],
            outputs: vec![port("output", PortDirection::Output)],
            configuration: vec![],
        });
        kind.inputs[0].temporal = conduit_core::PortTemporal::Flow { closes: true };
        kind.inputs[0].abnormal_kind = Some(kind_id("test/terminal"));
        kind.outputs[0].temporal = conduit_core::PortTemporal::Flow { closes: true };
        kind.outputs[0].abnormal_kind = Some(kind_id("test/terminal"));
        kind.semantic_laws
            .push(KindSemanticLaw::TerminalTransduction(
                TerminalTransductionProfile {
                    input_port_id: port_id("input"),
                    output_port_id: port_id("output"),
                    normal_close: NormalCloseTransduction::PropagateAfterDrain,
                    abnormal,
                    cancellation: CancellationTransduction::NotCancellable,
                },
            ));
        let mut profiles = ProfileCatalog::new();
        profiles.insert_kind(kind).unwrap();
        let startup = profiles.startup_catalog().unwrap();
        let source = "form main {\n transform: test/terminal-transform\n}\n";
        let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
        let expanded = expand_canonical_form(&checked, "main", &profiles).unwrap();
        let planned_form = crate::parse(source, &profiles).unwrap();
        planned_form.validate_identities().unwrap();
        assert_eq!(
            planned_form.gears[0].terminal_transductions,
            expanded.gears[0].terminal_transductions
        );
        (expanded, planned_form.checked_form_id)
    }

    let (propagating, propagating_checked) =
        expand(AbnormalTerminalTransduction::PropagateAfterDrain);
    assert_eq!(
        propagating.gears[0].terminal_transductions,
        vec![TerminalTransductionProfile {
            input_port_id: port_id("input"),
            output_port_id: port_id("output"),
            normal_close: NormalCloseTransduction::PropagateAfterDrain,
            abnormal: AbnormalTerminalTransduction::PropagateAfterDrain,
            cancellation: CancellationTransduction::NotCancellable,
        }]
    );
    let (recovering, recovering_checked) = expand(AbnormalTerminalTransduction::Recover);
    assert_ne!(propagating_checked, recovering_checked);
    assert_ne!(propagating.expanded_form_id, recovering.expanded_form_id);
}

#[test]
fn canonical_keep_uses_the_existing_retained_current_gear_and_direction_sugar() {
    let source = "form retained {\n cell: keep Scalar for this play\n}\n";
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert(KindProjection {
            kind_id: kind_id("state/latest"),
            kind_contract_revision: KindIdentity::from("state/latest-scalar@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("in"),
                value_kind: kind_id("value/scalar"),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            }],
            outputs: vec![PortDescriptor {
                port_id: port_id("out"),
                value_kind: kind_id("value/scalar"),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Current,
                abnormal_kind: None,
            }],
            configuration: vec![],
        })
        .unwrap();
    let expanded = expand_canonical_form(&checked, "retained", &profiles).unwrap();
    assert_eq!(expanded.gears.len(), 1);
    assert_eq!(expanded.gears[0].kind_id.as_str(), "state/latest");
}

#[test]
fn initialized_boolean_keep_lowers_to_exact_typed_state() {
    let source = "form retained {\n cell: keep Boolean(true) for this wake\n}\n";
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "retained", &ProfileCatalog::new()).unwrap();
    let [state] = expanded.gears.as_slice() else {
        panic!("initialized KEEP must lower to exactly one State Gear")
    };
    assert_eq!(state.kind_id.as_str(), conduit_core::STATE_VALUE_KIND);
    assert_eq!(
        state.kind_contract_revision.as_str(),
        conduit_core::STATE_VALUE_REVISION
    );
    assert_eq!(state.inputs[0].port_id, port_id("next"));
    assert_eq!(state.outputs[0].port_id, port_id("current"));
    assert_eq!(state.inputs[0].value_kind, state.outputs[0].value_kind);
    assert_eq!(
        state
            .configuration
            .iter()
            .find(|entry| entry.key == "retained-duration")
            .map(|entry| &entry.value),
        Some(&ConfigurationValue::Text("wake".into()))
    );
    assert!(matches!(
        state
            .configuration
            .iter()
            .find(|entry| entry.key == "initial")
            .map(|entry| &entry.value),
        Some(ConfigurationValue::Structured(_))
    ));
}

#[test]
fn optional_keep_lowers_omitted_and_present_initializers_to_none_and_some() {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let mut maximums = Vec::new();
    for (initializer, expected_tag) in [("", "none"), ("(true)", "some")] {
        let source =
            format!("form retained {{\n cell: keep Boolean?{initializer} for this play\n}}\n");
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let expanded = expand_canonical_form(&checked, "retained", &ProfileCatalog::new()).unwrap();
        let [state] = expanded.gears.as_slice() else {
            panic!("optional KEEP must lower to exactly one State Gear")
        };
        assert_eq!(state.kind_id.as_str(), conduit_core::STATE_VALUE_KIND);
        let ConfigurationValue::Structured(initial) = &state
            .configuration
            .iter()
            .find(|entry| entry.key == "initial")
            .unwrap()
            .value
        else {
            panic!("optional KEEP initializer must remain exact structured meaning")
        };
        let value =
            conduit_core::StructuredInfoValue::from_canonical_bytes(initial.canonical_value())
                .unwrap();
        let conduit_core::StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
            panic!("optional KEEP must use the canonical finite variant")
        };
        assert_eq!(tag, expected_tag);
        assert_eq!(state.inputs[0].value_kind, state.outputs[0].value_kind);
        maximums.push(
            state
                .configuration
                .iter()
                .find(|entry| entry.key == "maximum-bytes")
                .and_then(|entry| match entry.value {
                    ConfigurationValue::U64(value) => Some(value),
                    _ => None,
                })
                .unwrap(),
        );
    }
    assert_eq!(maximums, vec![100, 100]);
}

#[test]
fn optional_keep_refuses_an_explicit_bound_smaller_than_some_payload() {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let source = "form retained {\n cell: keep Boolean? <= 99B for this play\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let refusal = expand_canonical_form(&checked, "retained", &ProfileCatalog::new()).unwrap_err();
    assert_eq!(refusal.code, "CND-FRM-041");
    assert!(refusal.message.contains("admitted value envelope"));
}

#[test]
fn optional_dimensioned_keep_preserves_none_some_identity_bound_and_duration() {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();

    for (value_type, literal, quantity) in [
        (
            "Distance",
            "150cm",
            Quantity::new(150, QuantityUnit::Centimeter),
        ),
        (
            "Frequency",
            "440Hz",
            Quantity::new(440, QuantityUnit::Hertz),
        ),
    ] {
        let mut variants = Vec::new();
        let mut value_kinds = Vec::new();
        let mut maximums = Vec::new();
        for initializer in [String::new(), format!("({literal})")] {
            let source = format!(
                "form retained {{\n cell: keep {value_type}?{initializer} for this wake\n}}\n"
            );
            let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
            let expanded =
                expand_canonical_form(&checked, "retained", &ProfileCatalog::new()).unwrap();
            let [state] = expanded.gears.as_slice() else {
                panic!("optional dimensioned KEEP must lower to one typed State Gear")
            };
            let ConfigurationValue::Structured(initial) = &state
                .configuration
                .iter()
                .find(|entry| entry.key == "initial")
                .unwrap()
                .value
            else {
                panic!("optional dimensioned KEEP initializer must be structured")
            };
            let value =
                conduit_core::StructuredInfoValue::from_canonical_bytes(initial.canonical_value())
                    .unwrap();
            let conduit_core::StructuredInfoValueShape::Variant { tag, payload } = value.shape()
            else {
                panic!("optional dimensioned KEEP must be canonical none|some(T)")
            };
            if tag == "some" {
                let conduit_core::StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
                    panic!("optional quantity payload must remain one exact primitive leaf")
                };
                assert_eq!(bytes, quantity.encode());
            }
            variants.push(tag.to_string());
            assert_eq!(state.inputs[0].value_kind, state.outputs[0].value_kind);
            value_kinds.push(state.inputs[0].value_kind.clone());
            assert_eq!(
                state
                    .configuration
                    .iter()
                    .find(|entry| entry.key == "retained-duration")
                    .map(|entry| &entry.value),
                Some(&ConfigurationValue::Text("wake".into()))
            );
            maximums.push(
                state
                    .configuration
                    .iter()
                    .find(|entry| entry.key == "maximum-bytes")
                    .and_then(|entry| match entry.value {
                        ConfigurationValue::U64(value) => Some(value),
                        _ => None,
                    })
                    .unwrap(),
            );
        }
        assert_eq!(variants, ["none", "some"]);
        assert_eq!(value_kinds[0], value_kinds[1]);
        assert_eq!(maximums[0], maximums[1]);

        let source = format!(
            "form retained {{\n cell: keep {value_type}? <= {}B for this wake\n}}\n",
            maximums[0] - 1
        );
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
        let refusal =
            expand_canonical_form(&checked, "retained", &ProfileCatalog::new()).unwrap_err();
        assert_eq!(refusal.code, "CND-FRM-041");
        assert!(refusal.message.contains("admitted value envelope"));
    }
}

#[test]
fn pure_expression_lowers_to_one_exact_ordinary_gear() {
    let mut startup = StartupCatalog::new();
    for kind in ["test/u8-source", "test/u8-sink"] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
    }
    let u8_port = |name, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id("value/u8"),
        direction,
        temporal: conduit_core::PortTemporal::Value,
        abnormal_kind: None,
    };
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/u8-source"),
            kind_contract_revision: KindIdentity::from("test/u8-source@1"),
            inputs: vec![],
            outputs: vec![u8_port("out", PortDirection::Output)],
            configuration: vec![],
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/u8-sink"),
            kind_contract_revision: KindIdentity::from("test/u8-sink@1"),
            inputs: vec![u8_port("in", PortDirection::Input)],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    let source = "form arithmetic {\n source: test/u8-source\n sink: test/u8-sink\n source >> (. + 1) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "arithmetic", &profile).unwrap();
    let expression = expanded
        .gears
        .iter()
        .find(|gear| {
            gear.kind_contract_revision.as_str() == "conduitese/pure-expression-operation@1"
        })
        .expect("checked expression becomes an ordinary Gear");
    assert!(expression
        .kind_id
        .as_str()
        .starts_with("conduitese/pure-expression/"));
    assert_eq!(expression.inputs[0].value_kind.as_str(), "value/u8");
    assert_eq!(expression.outputs[0].value_kind.as_str(), "value/u8");
    assert_eq!(expanded.connections.len(), 2);
    expanded.validate_expansion().unwrap();
}

#[test]
fn expression_body_checks_and_expands_identically_to_its_explicit_form() {
    let concise = "form increment (\n    factor: U8 = 2\n    >> value: U8\n    result: U8 >>\n) = (. * factor)\n";
    let explicit = "form increment (\n    factor: U8 = 2\n    >> value: U8\n    result: U8 >>\n) {\n    value >> (. * factor) >> result\n}\n";
    let concise_checked =
        check_syntax_document(&parse_syntax_document(concise), &StartupCatalog::new()).unwrap();
    let explicit_checked =
        check_syntax_document(&parse_syntax_document(explicit), &StartupCatalog::new()).unwrap();
    assert_ne!(
        concise_checked.source_document_id,
        explicit_checked.source_document_id
    );
    assert_eq!(
        concise_checked.forms[0].checked_form_id,
        explicit_checked.forms[0].checked_form_id
    );

    let concise_expanded =
        expand_canonical_form_for_authoring(&concise_checked, "increment", &ProfileCatalog::new())
            .unwrap();
    let explicit_expanded =
        expand_canonical_form_for_authoring(&explicit_checked, "increment", &ProfileCatalog::new())
            .unwrap();
    assert_eq!(
        concise_expanded.expanded.expanded_form_id,
        explicit_expanded.expanded.expanded_form_id
    );
    assert_eq!(
        concise_expanded.expanded.gears,
        explicit_expanded.expanded.gears
    );
    assert_eq!(
        concise_expanded.expanded.connections,
        explicit_expanded.expanded.connections
    );
    assert_eq!(
        concise_expanded.input_bindings,
        explicit_expanded.input_bindings
    );
    assert_eq!(
        concise_expanded.output_bindings,
        explicit_expanded.output_bindings
    );
}

#[test]
fn expression_body_refuses_effectful_stateful_and_suspending_calls_without_escape_hatches() {
    let port = |name, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
        direction,
        temporal: conduit_core::PortTemporal::Value,
        abnormal_kind: None,
    };
    for (kind_name, law_index, ineligible_law) in [
        (
            "test/effect",
            0,
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::Observable),
        ),
        (
            "test/state",
            1,
            KindSemanticLaw::TemporalState(TemporalStateBehavior::Retained),
        ),
        (
            "test/suspend",
            5,
            KindSemanticLaw::Suspension(SuspensionBehavior::MaySuspend),
        ),
    ] {
        let mut laws = crate::pure_expression_semantic_laws();
        laws[law_index] = ineligible_law;
        let mut profile = ProfileCatalog::new();
        profile
            .insert_kind(Kind {
                kind_id: kind_id(kind_name),
                kind_contract_revision: KindIdentity::from(format!("{kind_name}@1")),
                startup_parameters: vec![],
                shorthand: Some((port_id("value"), port_id("result"))),
                inputs: vec![port("value", PortDirection::Input)],
                outputs: vec![port("result", PortDirection::Output)],
                configuration: vec![],
                semantic_laws: laws,
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: 8,
                },
            })
            .unwrap();
        let source = format!(
            "form dangerous (\n    >> value: Scalar\n    result: Scalar >>\n) = ({kind_name}(.))\n"
        );
        let checked = check_syntax_document(
            &parse_syntax_document(&source),
            &profile.startup_catalog().unwrap(),
        )
        .unwrap();
        let refusal =
            expand_canonical_form_for_authoring(&checked, "dangerous", &profile).unwrap_err();
        assert_eq!(refusal.code, "CND-FRM-046");
        assert!(refusal
            .message
            .contains("ineligible for pure expression use"));
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("ChatMessage", kind_id("chat/message@1"))
        .unwrap();
    for signature in [
        KindSignature {
            kind: "test/source".into(),
            startup_parameters: vec![],
        },
        KindSignature {
            kind: "test/pass".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "count".into(),
                value_type: "Count".into(),
                default: Some("1".into()),
            }],
        },
        KindSignature {
            kind: "test/sink".into(),
            startup_parameters: vec![],
        },
        KindSignature {
            kind: "test/use-pool".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "members".into(),
                value_type: "Pool".into(),
                default: None,
            }],
        },
    ] {
        startup.insert(signature).unwrap();
    }
    let mut profile = ProfileCatalog::new();
    for definition in [
        KindProjection {
            kind_id: kind_id("test/source"),
            kind_contract_revision: KindIdentity::from("test/source@1"),
            inputs: vec![],
            outputs: vec![port("out", PortDirection::Output)],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/use-pool"),
            kind_contract_revision: KindIdentity::from("test/use-pool@1"),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/pass"),
            kind_contract_revision: KindIdentity::from("test/pass@1"),
            inputs: vec![port("in", PortDirection::Input)],
            outputs: vec![port("out", PortDirection::Output)],
            configuration: vec![KindConfigurationField {
                key: "count".into(),
                default_value: ConfigurationValue::U64(1),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: 8,
                },
            }],
        },
        KindProjection {
            kind_id: kind_id("test/sink"),
            kind_contract_revision: KindIdentity::from("test/sink@1"),
            inputs: vec![port("in", PortDirection::Input)],
            outputs: vec![],
            configuration: vec![],
        },
    ] {
        profile.insert_kind(canonical_kind(definition)).unwrap();
    }
    (startup, profile)
}

#[test]
fn selected_canonical_back_changes_only_expansion_identity_and_records_exact_provenance() {
    let (mut startup, mut profile) = catalogs();
    startup
        .insert(KindSignature {
            kind: "test/high".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let high = KindProjection {
        kind_id: kind_id("test/high"),
        kind_contract_revision: KindIdentity::from("test/high@1"),
        inputs: vec![port("in", PortDirection::Input)],
        outputs: vec![port("out", PortDirection::Output)],
        configuration: vec![],
    };
    profile.insert_kind(canonical_kind(high.clone())).unwrap();

    let user = check_syntax_document(
        &parse_syntax_document(
            "form main {\n source: test/source\n high: test/high\n sink: test/sink\n source >> high >> sink\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let direct = expand_canonical_form(&user, "main", &profile).unwrap();

    let back_document = check_syntax_document(
        &parse_syntax_document(
            "form test/high (\n in: test/value >> out: test/value\n) {\n leaf: test/pass\n in >> leaf >> out\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let mut backs = crate::CanonicalBackCatalog::new();
    backs
        .insert(
            profile.canonical_kind(&high.kind_id).unwrap(),
            &back_document,
            "test/high",
        )
        .unwrap();
    let recursive =
        crate::expand_canonical_form_with_backs(&user, "main", &profile, &backs).unwrap();

    assert_eq!(direct.source_document_id, recursive.source_document_id);
    assert_eq!(direct.checked_form_id, recursive.checked_form_id);
    assert_ne!(direct.expanded_form_id, recursive.expanded_form_id);
    assert!(direct.realization_backs.is_empty());
    assert_eq!(recursive.realization_backs.len(), 1);
    assert_eq!(recursive.realization_backs[0].invocation_path, "main/high");
    assert_eq!(recursive.realization_backs[0].kind_id.as_str(), "test/high");
    assert_eq!(
        recursive.realization_backs[0].source_document_id,
        back_document.source_document_id
    );
    assert!(recursive
        .gears
        .iter()
        .any(|gear| gear.gear_id.as_str() == "main/high/leaf"));
    recursive.validate_expansion().unwrap();
}

#[test]
fn canonical_back_refuses_a_front_that_differs_from_the_high_level_kind() {
    let (startup, profile) = catalogs();
    let document = check_syntax_document(
        &parse_syntax_document("form wrong {\n leaf: test/source\n}\n"),
        &startup,
    )
    .unwrap();
    let mut backs = crate::CanonicalBackCatalog::new();
    let error = backs
        .insert(
            profile.canonical_kind(&kind_id("test/pass")).unwrap(),
            &document,
            "wrong",
        )
        .unwrap_err();
    assert_eq!(
        error,
        crate::CanonicalBackError::FaceMismatch("test/pass".into())
    );
}

#[test]
fn exact_back_admission_refuses_stale_source_and_checked_form_identities() {
    let (startup, profile) = catalogs();
    let high = profile.canonical_kind(&kind_id("test/pass")).unwrap();
    let document = check_syntax_document(
        &parse_syntax_document(
            "form test/pass (\n count: Count = 1\n in: test/value >> out: test/value\n) {\n leaf: test/pass(count)\n in >> leaf >> out\n}\n",
        ),
        &startup,
    )
    .unwrap();
    let checked_form_id = document.forms[0].checked_form_id.clone();

    let mut backs = crate::CanonicalBackCatalog::new();
    assert!(matches!(
        backs.insert_exact(
            high,
            &[conduit_core::FrontStartupParameter {
                name: "count".into(),
                value_type: conduit_core::kind_id("value/count"),
                has_default: true,
            }],
            &document,
            "test/pass",
            &conduit_core::SourceDocumentId::from("stale-source"),
            &checked_form_id,
        ),
        Err(crate::CanonicalBackError::StaleSourceDocument { .. })
    ));
    assert!(matches!(
        backs.insert_exact(
            high,
            &[conduit_core::FrontStartupParameter {
                name: "count".into(),
                value_type: conduit_core::kind_id("value/count"),
                has_default: true,
            }],
            &document,
            "test/pass",
            &document.source_document_id,
            &conduit_core::CheckedFormId::from("stale-form"),
        ),
        Err(crate::CanonicalBackError::StaleCheckedForm { .. })
    ));

    backs
        .insert_exact(
            high,
            &[conduit_core::FrontStartupParameter {
                name: "count".into(),
                value_type: conduit_core::kind_id("value/count"),
                has_default: true,
            }],
            &document,
            "test/pass",
            &document.source_document_id,
            &checked_form_id,
        )
        .unwrap();
}

fn expand(source: &str, root: &str) -> crate::ExpandedCanonicalForm {
    let (startup, profile) = catalogs();
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, &startup).expect("source checks");
    expand_canonical_form(&checked, root, &profile).expect("source expands")
}

fn terminal_catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    let definitions = [
        KindProjection {
            kind_id: kind_id("test/closing-source"),
            kind_contract_revision: KindIdentity::from("test/closing-source@1"),
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("out"),
                value_kind: kind_id("test/value"),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Flow { closes: true },
                abnormal_kind: Some(kind_id("test/fault")),
            }],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/standing-source"),
            kind_contract_revision: KindIdentity::from("test/standing-source@1"),
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("out"),
                value_kind: kind_id("test/value"),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Flow { closes: false },
                abnormal_kind: None,
            }],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/unit-sink"),
            kind_contract_revision: KindIdentity::from("test/unit-sink@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("in"),
                value_kind: kind_id(conduit_core::UNIT_INFO_ID),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/fault-sink"),
            kind_contract_revision: KindIdentity::from("test/fault-sink@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("in"),
                value_kind: kind_id("test/fault"),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/deadline"),
            kind_contract_revision: KindIdentity::from("test/deadline@1"),
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("out"),
                value_kind: kind_id(conduit_core::CANCELLATION_REQUEST_INFO_ID),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: None,
            }],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/cancellable-work"),
            kind_contract_revision: KindIdentity::from("test/cancellable-work@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("halt"),
                value_kind: kind_id(conduit_core::CANCELLATION_REQUEST_INFO_ID),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![PortDescriptor {
                port_id: port_id("result"),
                value_kind: kind_id("test/value"),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: Some(kind_id("test/cancelled")),
            }],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/typed-but-not-cancellable-work"),
            kind_contract_revision: KindIdentity::from("test/typed-but-not-cancellable-work@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("halt"),
                value_kind: kind_id(conduit_core::CANCELLATION_REQUEST_INFO_ID),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![PortDescriptor {
                port_id: port_id("result"),
                value_kind: kind_id("test/value"),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: Some(kind_id("test/cancelled")),
            }],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/name-only-cancellable-work"),
            kind_contract_revision: KindIdentity::from("test/name-only-cancellable-work@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("cancel"),
                value_kind: kind_id(conduit_core::UNIT_INFO_ID),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/ambiguous-cancellable-work"),
            kind_contract_revision: KindIdentity::from("test/ambiguous-cancellable-work@1"),
            inputs: vec!["first", "second"]
                .into_iter()
                .map(|name| PortDescriptor {
                    port_id: port_id(name),
                    value_kind: kind_id(conduit_core::CANCELLATION_REQUEST_INFO_ID),
                    direction: PortDirection::Input,
                    temporal: conduit_core::PortTemporal::Value,
                    abnormal_kind: None,
                })
                .collect(),
            outputs: vec![],
            configuration: vec![],
        },
        KindProjection {
            kind_id: kind_id("test/unit-pass"),
            kind_contract_revision: KindIdentity::from("test/unit-pass@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("in"),
                value_kind: kind_id(conduit_core::UNIT_INFO_ID),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![PortDescriptor {
                port_id: port_id("out"),
                value_kind: kind_id(conduit_core::UNIT_INFO_ID),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Value,
                abnormal_kind: None,
            }],
            configuration: vec![],
        },
    ];
    for definition in definitions {
        startup
            .insert(KindSignature {
                kind: definition.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        let mut kind = canonical_kind(definition);
        if kind.kind_id.as_str() == "test/cancellable-work" {
            kind.semantic_laws
                .push(KindSemanticLaw::TerminalTransduction(
                    TerminalTransductionProfile {
                        input_port_id: port_id("halt"),
                        output_port_id: port_id("result"),
                        normal_close: NormalCloseTransduction::NotAccepted,
                        abnormal: AbnormalTerminalTransduction::NotAccepted,
                        cancellation: CancellationTransduction::Request {
                            disposition_kind: kind_id("test/cancelled"),
                        },
                    },
                ));
        } else if kind.kind_id.as_str() == "test/typed-but-not-cancellable-work" {
            kind.semantic_laws
                .push(KindSemanticLaw::TerminalTransduction(
                    TerminalTransductionProfile {
                        input_port_id: port_id("halt"),
                        output_port_id: port_id("result"),
                        normal_close: NormalCloseTransduction::NotAccepted,
                        abnormal: AbnormalTerminalTransduction::NotAccepted,
                        cancellation: CancellationTransduction::NotCancellable,
                    },
                ));
        }
        profile.insert_kind(kind).unwrap();
    }
    let mut recovery = canonical_kind(KindProjection {
        kind_id: kind_id("test/fault-recovery"),
        kind_contract_revision: KindIdentity::from("test/fault-recovery@1"),
        inputs: vec![PortDescriptor {
            port_id: port_id("terminal"),
            value_kind: kind_id("test/fault"),
            direction: PortDirection::Input,
            temporal: conduit_core::PortTemporal::Value,
            abnormal_kind: Some(kind_id("test/fault")),
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("recovered"),
            value_kind: kind_id(conduit_core::UNIT_INFO_ID),
            direction: PortDirection::Output,
            temporal: conduit_core::PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: vec![],
    });
    recovery
        .semantic_laws
        .push(KindSemanticLaw::TerminalTransduction(
            TerminalTransductionProfile {
                input_port_id: port_id("terminal"),
                output_port_id: port_id("recovered"),
                normal_close: NormalCloseTransduction::NotAccepted,
                abnormal: AbnormalTerminalTransduction::Recover,
                cancellation: CancellationTransduction::NotCancellable,
            },
        ));
    startup
        .insert(KindSignature {
            kind: "test/fault-recovery".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    profile.insert_kind(recovery).unwrap();
    (startup, profile)
}

#[test]
fn terminal_projections_lower_to_distinct_typed_connection_tracks() {
    let (startup, profile) = terminal_catalogs();
    let source = "form main {\n source: test/closing-source\n finish: test/unit-sink\n explain: test/fault-sink\n source| >> finish\n source! >> explain\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "main", &profile).unwrap();

    assert_eq!(expanded.connections.len(), 2);
    let normal = expanded
        .connections
        .iter()
        .find(|connection| connection.track == conduit_core::ConnectionTrack::NormalClose)
        .expect("normal close is an exact graph track");
    assert_eq!(normal.value_kind.as_str(), conduit_core::UNIT_INFO_ID);
    assert_eq!(normal.temporal, conduit_core::PortTemporal::Value);
    let abnormal = expanded
        .connections
        .iter()
        .find(|connection| connection.track == conduit_core::ConnectionTrack::AbnormalTerminal)
        .expect("abnormal terminal truth is an exact graph track");
    assert_eq!(abnormal.value_kind.as_str(), "test/fault");
    assert_eq!(abnormal.temporal, conduit_core::PortTemporal::Value);
    assert_ne!(normal, abnormal);
}

#[test]
fn normal_close_projection_refuses_a_nonclosing_flow() {
    let (startup, profile) = terminal_catalogs();
    let source = "form main {\n source: test/standing-source\n finish: test/unit-sink\n source| >> finish\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-046");
    assert!(error.message.contains("legal only for a closing Flow"));
}

#[test]
fn abnormal_projection_requires_the_exact_source_fore_terminal_kind() {
    let (startup, profile) = terminal_catalogs();
    let undeclared = "form main {\n source: test/standing-source\n explain: test/fault-sink\n source! >> explain\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(undeclared), &startup).unwrap();
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-046");
    assert!(error.message.contains("exact abnormal terminal type"));

    let wrong_sink = "form main {\n source: test/closing-source\n finish: test/unit-sink\n source! >> finish\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(wrong_sink), &startup).unwrap();
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-045");
    assert!(error
        .message
        .contains("incompatible abnormal-terminal contracts"));
}

#[test]
fn semantic_cancellation_is_an_ordinary_cord_to_an_exact_declared_fore_control() {
    let (startup, profile) = terminal_catalogs();
    let source = "form main {\n deadline: test/deadline\n work: test/cancellable-work\n deadline >> work~\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "main", &profile).unwrap();
    assert_eq!(expanded.connections.len(), 1);
    let cancellation = &expanded.connections[0];
    assert_eq!(cancellation.track, conduit_core::ConnectionTrack::Payload);
    assert_eq!(
        cancellation.value_kind.as_str(),
        conduit_core::CANCELLATION_REQUEST_INFO_ID
    );
    assert_eq!(cancellation.sink_gear_id.as_str(), "main/work");
    assert_eq!(cancellation.sink_port_id.as_str(), "halt");

    let unavailable =
        "form main {\n deadline: test/deadline\n work: test/unit-sink\n deadline >> work~\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(unavailable), &startup).unwrap();
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-046");
    assert!(error
        .message
        .contains("does not declare CancellationTransduction::Request"));

    let misleading_name = "form main {\n deadline: test/deadline\n work: test/name-only-cancellable-work\n deadline >> work~\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(misleading_name), &startup).unwrap();
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-046");
    assert!(error
        .message
        .contains("does not declare CancellationTransduction::Request"));

    let typed_only = "form main {\n deadline: test/deadline\n work: test/typed-but-not-cancellable-work\n deadline >> work~\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(typed_only), &startup).unwrap();
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-046");
    assert!(error
        .message
        .contains("does not declare CancellationTransduction::Request"));

    let ambiguous = "form main {\n deadline: test/deadline\n work: test/ambiguous-cancellable-work\n deadline >> work~\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(ambiguous), &startup).unwrap();
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-046");
    assert!(error
        .message
        .contains("does not declare CancellationTransduction::Request"));
}

#[test]
fn terminal_projection_survives_a_nested_form_input_boundary() {
    let (startup, profile) = terminal_catalogs();
    let source = "form relay (\n input: test/value...| >> closed: Unit\n) {\n finish: test/unit-pass\n input| >> finish >> closed\n}\n\nform main {\n source: test/closing-source\n relay: relay\n sink: test/unit-sink\n source >> relay.input\n relay.closed >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "main", &profile).unwrap();
    let terminal = expanded
        .connections
        .iter()
        .find(|connection| connection.track == conduit_core::ConnectionTrack::NormalClose)
        .expect("nested Form boundary retains the normal-close track");
    assert_eq!(terminal.source_gear_id.as_str(), "main/source");
    assert_eq!(terminal.sink_gear_id.as_str(), "main/relay/finish");
    assert_eq!(terminal.value_kind.as_str(), conduit_core::UNIT_INFO_ID);
}

#[test]
fn unhandled_child_abnormal_is_an_exact_separate_containing_form_export() {
    let (startup, profile) = terminal_catalogs();
    let source = "form child (\n output: test/value...| >>\n) {\n work: test/closing-source\n work >> output\n}\n\nform main {\n child: child\n explain: test/fault-sink\n child! >> explain\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();

    let child = expand_canonical_form_for_authoring(&checked, "child", &profile).unwrap();
    assert_eq!(child.output_bindings.len(), 1);
    assert_eq!(
        child.output_bindings[0].track,
        conduit_core::ConnectionTrack::Payload
    );
    let abnormal = child
        .abnormal_export
        .expect("unrecovered child terminal is a checked Form-level export");
    assert_eq!(abnormal.value_kind.as_str(), "test/fault");
    assert_eq!(abnormal.gear_id.as_str(), "child/work");
    assert_eq!(abnormal.gear_port_id.as_str(), "out");

    let main = expand_canonical_form(&checked, "main", &profile).unwrap();
    let terminal = main
        .connections
        .iter()
        .find(|connection| connection.track == conduit_core::ConnectionTrack::AbnormalTerminal)
        .expect("outer child! resolves to the exact inner terminal origin");
    assert_eq!(terminal.source_gear_id.as_str(), "main/child/work");
    assert_eq!(terminal.source_port_id.as_str(), "out");
    assert_eq!(terminal.value_kind.as_str(), "test/fault");
    assert_eq!(terminal.sink_gear_id.as_str(), "main/explain");
}

#[test]
fn inferred_form_abnormal_composes_recursively_without_payload_binding() {
    let (startup, profile) = terminal_catalogs();
    let source = "form leaf {\n work: test/closing-source\n}\n\nform middle {\n leaf: leaf\n}\n\nform main {\n middle: middle\n explain: test/fault-sink\n middle! >> explain\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "main", &profile).unwrap();
    let terminal = expanded
        .connections
        .iter()
        .find(|connection| connection.track == conduit_core::ConnectionTrack::AbnormalTerminal)
        .expect("outer Form projects its recursively inferred typed abnormal export");
    assert_eq!(terminal.source_gear_id.as_str(), "main/middle/leaf/work");
    assert_eq!(terminal.value_kind.as_str(), "test/fault");
}

#[test]
fn exact_recovery_discharges_the_containing_form_abnormal_export() {
    let (startup, profile) = terminal_catalogs();
    let source = "form child {\n work: test/closing-source\n recover: test/fault-recovery\n work! >> recover\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let child = expand_canonical_form_for_authoring(&checked, "child", &profile).unwrap();
    assert!(child.abnormal_export.is_none());
    assert_eq!(child.expanded.connections.len(), 1);
    assert_eq!(
        child.expanded.connections[0].track,
        conduit_core::ConnectionTrack::AbnormalTerminal
    );
}

#[test]
fn multiple_unresolved_child_abnormals_refuse_implicit_fanin() {
    let (startup, profile) = terminal_catalogs();
    let source = "form child {\n first: test/closing-source\n second: test/closing-source\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let error = expand_canonical_form_for_authoring(&checked, "child", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-054");
    assert!(error
        .message
        .contains("2 unresolved abnormal terminal origins"));
}

#[test]
fn multiple_abnormal_origins_cannot_claim_one_implicit_recovery_input() {
    let (startup, profile) = terminal_catalogs();
    let source = "form child {\n first: test/closing-source\n second: test/closing-source\n recover: test/fault-recovery\n first! >> recover\n second! >> recover\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let error = expand_canonical_form_for_authoring(&checked, "child", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-054");
    assert!(error.message.contains("one exact source"));
}

#[derive(Clone, Copy, Debug)]
enum NestedTerminalDriver {
    AbnormalSource,
    Observer,
    Recovery { phase: u8 },
}

impl StepBack<1> for NestedTerminalDriver {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        matches!(self, Self::Recovery { .. }).then_some(AssignedTerminalTransduction {
            input: KernelPortId(0),
            output: KernelPortId(0),
            normal_close: AssignedNormalCloseTransduction::NotAccepted,
            abnormal: AssignedAbnormalTransduction::Recover,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        })
    }

    fn step(&mut self, io: &mut StepIo<1>, _input_bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::AbnormalSource => StepOutcome::Abnormal {
                port: KernelPortId(0),
                terminal: CanonicalValue::new(&[1, 0, 0x34, 0x12]).unwrap(),
            },
            Self::Observer => {
                if io.input(KernelPortId(0)).is_some() {
                    io.consume(KernelPortId(0)).unwrap();
                    StepOutcome::Progress
                } else if io.input_closed(KernelPortId(0)) {
                    io.consume_closed(KernelPortId(0)).unwrap();
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
            Self::Recovery { phase } if *phase == 0 => {
                if io.input(KernelPortId(0)).is_none() {
                    return StepOutcome::Await;
                }
                io.consume(KernelPortId(0)).unwrap();
                *phase = 1;
                StepOutcome::Progress
            }
            Self::Recovery { .. } => StepOutcome::Complete,
        }
    }
}

fn nested_terminal_scheduler(
    sink: NestedTerminalDriver,
) -> FixedScheduler<NestedTerminalDriver, FixedValueStore<2, 4>, FixedSignLog<16>, 2, 1, 1, 1, 2, 1>
{
    let mut routes = FixedRoutes::<2, 1>::new(1);
    routes
        .install(
            NodeId(0),
            KernelPortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(1), KernelPortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut scheduler = FixedScheduler::new(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 3,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 3,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (NodeId(0), KernelPortId(0)),
            (NodeId(1), KernelPortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 4,
                pressure_policy: Default::default(),
            },
        )
        .with_track(AssignedConnectionTrack::AbnormalTerminal)],
        routes,
        [NestedTerminalDriver::AbnormalSource, sink],
        FixedValueStore::new(4).unwrap(),
        FixedSignLog::new((16 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32)
            .unwrap(),
    )
    .unwrap();
    scheduler
        .bind_terminal_transductions([[None], [sink.terminal_transduction()]])
        .unwrap();
    scheduler
}

#[test]
fn nested_form_observation_does_not_hide_unhandled_root_abnormal() {
    let (startup, profile) = terminal_catalogs();
    let source = "form child {\n work: test/closing-source\n}\n\nform main {\n child: child\n explain: test/fault-sink\n child! >> explain\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "main", &profile).unwrap();
    assert_eq!(expanded.connections.len(), 1);
    assert_eq!(
        expanded.connections[0].source_gear_id.as_str(),
        "main/child/work"
    );
    assert_eq!(
        expanded.connections[0].track,
        conduit_core::ConnectionTrack::AbnormalTerminal
    );

    let mut scheduler = nested_terminal_scheduler(NestedTerminalDriver::Observer);
    let error = (0..6)
        .find_map(|_| scheduler.step().err())
        .expect("observed child abnormal remains unresolved at root drain");
    assert!(matches!(
        error,
        SchedulerError::SemanticAbnormal {
            node: NodeId(0),
            port: KernelPortId(0),
            ..
        }
    ));
}

#[test]
fn nested_form_exact_recovery_clears_the_runtime_abnormal_obligation() {
    let (startup, profile) = terminal_catalogs();
    let source = "form child {\n work: test/closing-source\n recover: test/fault-recovery\n work! >> recover\n}\n\nform main {\n child: child\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "main", &profile).unwrap();
    assert_eq!(expanded.connections.len(), 1);
    assert_eq!(
        expanded.connections[0].source_gear_id.as_str(),
        "main/child/work"
    );
    assert_eq!(
        expanded.connections[0].sink_gear_id.as_str(),
        "main/child/recover"
    );

    let mut scheduler = nested_terminal_scheduler(NestedTerminalDriver::Recovery { phase: 0 });
    let mut status = SchedulerStatus::Progress { node: NodeId(0) };
    for _ in 0..6 {
        status = scheduler.step().unwrap();
        if status == SchedulerStatus::Drained {
            break;
        }
    }
    assert_eq!(status, SchedulerStatus::Drained);
}

#[test]
fn parameterized_form_flattens_to_ordinary_primitive_graph() {
    let source = "form relay (\n count: Count = 1\n input: test/value >> output: test/value\n) {\n pass: test/pass(count)\n input >> pass >> output\n}\n\nform main {\n source: test/source\n relay: relay(2)\n sink: test/sink\n source >> relay >> sink\n}\n";
    let expanded = expand(source, "main");

    assert_eq!(
        expanded
            .gears
            .iter()
            .map(|gear| gear.gear_id.as_str())
            .collect::<Vec<_>>(),
        ["main/relay/pass", "main/sink", "main/source"]
    );
    let pass = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "test/pass")
        .unwrap();
    assert_eq!(pass.configuration[0].value, ConfigurationValue::U64(2));
    assert_eq!(expanded.connections.len(), 2);
    assert_eq!(expanded.provenance[0].source_form, "relay");
    assert_ne!(
        expanded.checked_form_id.as_str(),
        expanded.expanded_form_id.as_str()
    );
}

#[test]
fn two_explicit_consumers_share_one_exact_expanded_pool_reference() {
    let source = "form chat/peer (\n recv: ChatMessage...| >> send: ChatMessage...|\n) {\n}\n\nform consumer (\n members: Pool\n) {\n use: test/use-pool(members)\n}\n\nform room {\n pool peers: chat/peer(size = 2)\n left: consumer(peers)\n right: consumer(peers)\n}\n";
    let expanded = expand(source, "room");
    assert_eq!(expanded.shared_pools.len(), 1);
    let pool = &expanded.shared_pools[0];
    assert_eq!(pool.pool_id.as_str(), "room/peers");
    assert_eq!(pool.maximum_members, 2);
    assert_eq!(
        pool.consumers
            .iter()
            .map(|consumer| consumer.as_str())
            .collect::<Vec<_>>(),
        ["room/left/use", "room/right/use"]
    );
    assert!(expanded.gears.iter().all(|gear| {
        gear.pool_references == vec![conduit_core::SharedPoolId::from("room/peers")]
    }));
    expanded.validate_expansion().unwrap();

    let mut mutated = expanded;
    mutated.shared_pools[0].maximum_members = 3;
    assert!(mutated.validate_expansion().is_err());
}

#[test]
fn pool_name_is_not_ambiently_captured_by_nested_forms_or_graph_cords() {
    let (startup, profile) = catalogs();
    let ambient = parse_syntax_document(
        "form chat/peer {\n}\n\nform consumer {\n use: test/use-pool(peers)\n}\n\nform room {\n pool peers: chat/peer(size = 2)\n child: consumer\n}\n",
    );
    let checked = check_syntax_document(&ambient, &startup).unwrap();
    let diagnostic = expand_canonical_form(&checked, "room", &profile).unwrap_err();
    assert_eq!(diagnostic.code, "CND-FRM-041");

    let implicit = parse_syntax_document(
        "form chat/peer {\n}\n\nform room {\n pool peers: chat/peer(size = 2)\n source: test/source\n source >> peers\n}\n",
    );
    let checked = check_syntax_document(&implicit, &startup).unwrap();
    let diagnostic = expand_canonical_form(&checked, "room", &profile).unwrap_err();
    assert_eq!(diagnostic.code, "CND-FRM-042");
}

#[test]
fn nested_expansion_and_source_reordering_have_deterministic_identity() {
    let first = "form inner (\n input: test/value >> output: test/value\n) {\n pass: test/pass\n input >> pass >> output\n}\n\nform outer (\n input: test/value >> output: test/value\n) {\n inner: inner\n input >> inner >> output\n}\n\nform main {\n source: test/source\n outer: outer\n sink: test/sink\n source >> outer >> sink\n}\n";
    let reordered = "form main {\n source >> outer >> sink\n sink: test/sink\n outer: outer\n source: test/source\n}\n\nform outer (\n input: test/value >> output: test/value\n) {\n input >> inner >> output\n inner: inner\n}\n\nform inner (\n input: test/value >> output: test/value\n) {\n input >> pass >> output\n pass: test/pass\n}\n";
    let first = expand(first, "main");
    let reordered = expand(reordered, "main");
    assert_eq!(first.checked_form_id, reordered.checked_form_id);
    assert_eq!(first.expanded_form_id, reordered.expanded_form_id);
    assert_eq!(first.gears, reordered.gears);
    assert_eq!(first.connections, reordered.connections);
}

#[test]
fn two_uses_share_one_form_definition_but_have_distinct_occurrence_paths() {
    let source = "form relay (\n input: test/value >> output: test/value\n) {\n pass: test/pass\n input >> pass >> output\n}\n\nform main {\n source: test/source\n left: relay\n right: relay\n left_sink: test/sink\n right_sink: test/sink\n source >> left >> left_sink\n source >> right >> right_sink\n}\n";
    let expanded = expand(source, "main");
    let relay_gears = expanded
        .provenance
        .iter()
        .filter(|item| item.source_form == "relay")
        .collect::<Vec<_>>();

    assert_eq!(relay_gears.len(), 2);
    assert_eq!(relay_gears[0].source_gear, "pass");
    assert_eq!(relay_gears[1].source_gear, "pass");
    assert_eq!(relay_gears[0].form_path, ["main", "left"]);
    assert_eq!(relay_gears[1].form_path, ["main", "right"]);
    assert_ne!(relay_gears[0].gear_id, relay_gears[1].gear_id);
    expanded.validate_expansion().unwrap();
}

#[test]
fn recursion_and_expansion_depth_fail_with_distinct_diagnostics() {
    let (startup, profile) = catalogs();
    let recursive = parse_syntax_document("form a {\n child: b\n}\n\nform b {\n child: a\n}\n");
    let checked = check_syntax_document(&recursive, &startup).unwrap();
    let error = expand_canonical_form(&checked, "a", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-035");
    assert!(error.message.contains("a > b > a"));

    let mut source = String::new();
    for index in 0..=crate::MAXIMUM_FORM_NESTING_DEPTH + 1 {
        source.push_str(&format!("form f{index} {{\n"));
        if index <= crate::MAXIMUM_FORM_NESTING_DEPTH {
            source.push_str(&format!(" child: f{}\n", index + 1));
        }
        source.push_str("}\n\n");
    }
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let error = expand_canonical_form(&checked, "f0", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-034");
}

#[test]
fn reusable_form_without_declared_shorthand_requires_named_port() {
    let source = "form source (\n value: test/value >>\n) {\n primitive: test/source\n primitive >> value\n}\n\nform main {\n source: source\n sink: test/sink\n source >> sink\n}\n";
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let error = expand_canonical_form(&checked, "main", &profile).unwrap_err();
    assert_eq!(error.code, "CND-FRM-044");
}

#[test]
fn primitive_contract_bounds_and_front_types_fail_closed() {
    let bounded = "form main {\n source: test/source\n pass: test/pass(9)\n sink: test/sink\n source >> pass >> sink\n}\n";
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(bounded), &startup).unwrap();
    assert_eq!(
        expand_canonical_form(&checked, "main", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-040"
    );

    let wrong_front = "form relay (\n input: wrong/value >> output: wrong/value\n) {\n pass: test/pass\n input >> pass >> output\n}\n\nform main {\n source: test/source\n relay: relay\n sink: test/sink\n source >> relay >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(wrong_front), &startup).unwrap();
    assert_eq!(
        expand_canonical_form(&checked, "main", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-045"
    );
}

#[test]
fn public_input_fanout_flattens_to_explicit_ordinary_connections() {
    let source = "form fan (\n >> input: test/value\n) {\n left: test/pass\n right: test/pass\n input >> left\n input >> right\n}\n\nform main {\n source: test/source\n fan: fan\n source >> fan.input\n}\n";
    let expanded = expand(source, "main");
    assert_eq!(expanded.connections.len(), 2);
    assert!(expanded.connections.iter().all(|connection| {
        connection.source_gear_id.as_str() == "main/source"
            && matches!(
                connection.sink_gear_id.as_str(),
                "main/fan/left" | "main/fan/right"
            )
    }));
}

#[test]
fn expanded_identity_rejects_graph_contract_and_provenance_mutation() {
    let source = "form main {\n source: test/source\n sink: test/sink\n source >> sink\n}\n";
    let baseline = expand(source, "main");

    let mut gear = baseline.clone();
    gear.gears[0].kind_contract_revision = KindIdentity::from("mutated@1");
    assert_eq!(gear.validate_expansion().unwrap_err().code, "CND-FRM-049");

    let mut cord = baseline.clone();
    cord.connections[0].sink_port_id = port_id("mutated");
    assert_eq!(cord.validate_expansion().unwrap_err().code, "CND-FRM-049");

    let mut provenance = baseline;
    provenance.provenance[0].source_gear = "substituted".into();
    assert_eq!(
        provenance.validate_expansion().unwrap_err().code,
        "CND-FRM-049"
    );

    let mut span = expand(source, "main");
    span.provenance[0].source_span.start += 1;
    assert_eq!(span.validate_expansion().unwrap_err().code, "CND-FRM-049");
}

#[test]
fn inline_reusable_and_primitive_gears_expand_without_a_parallel_path() {
    let source = "form relay (\n input: test/value >> output: test/value\n) {\n input >> test/pass >> output\n}\n\nform main {\n test/source >> relay() >> test/sink\n}\n";
    let expanded = expand(source, "main");
    assert_eq!(expanded.gears.len(), 3);
    assert_eq!(expanded.connections.len(), 2);
    assert!(expanded
        .provenance
        .iter()
        .all(|row| row.source_gear.starts_with("inline-")));
}

#[test]
fn front_binding_preserves_flow_closure_and_current_observation_contracts() {
    let mut startup = StartupCatalog::new();
    for gear in ["state/count", "test/ticks", "test/current"] {
        startup
            .insert(KindSignature {
                kind: gear.into(),
                startup_parameters: vec![],
            })
            .unwrap();
    }
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: kind_id("state/count"),
            kind_contract_revision: KindIdentity::from("state/count@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("bump"),
                value_kind: kind_id("value/tick@1"),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            }],
            outputs: vec![PortDescriptor {
                port_id: port_id("value"),
                value_kind: kind_id("value/count"),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Current,
                abnormal_kind: None,
            }],
            configuration: vec![],
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/ticks"),
            kind_contract_revision: KindIdentity::from("test/ticks@1"),
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("tick"),
                value_kind: kind_id("value/tick@1"),
                direction: PortDirection::Output,
                temporal: conduit_core::PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            }],
            configuration: vec![],
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/current"),
            kind_contract_revision: KindIdentity::from("test/current@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("value"),
                value_kind: kind_id("value/count"),
                direction: PortDirection::Input,
                temporal: conduit_core::PortTemporal::Current,
                abnormal_kind: None,
            }],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    let source = "form count (\n    bump: Tick...| >> value: $Count\n) {\n    gear: state/count\n    bump >> gear.bump\n    gear.value >> value\n}\n\nform main {\n    ticks: test/ticks\n    count: count\n    show: test/current\n    ticks >> count >> show\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "main", &profile).unwrap();
    let count = checked
        .forms
        .iter()
        .find(|form| form.name == "count")
        .unwrap();
    let state = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "state/count")
        .unwrap();
    assert_eq!(count.checked_front(), state.checked_front());

    let mismatched = source.replace("Tick...|", "Tick...");
    let checked = check_syntax_document(&parse_syntax_document(&mismatched), &startup).unwrap();
    assert_eq!(
        expand_canonical_form(&checked, "main", &profile)
            .unwrap_err()
            .code,
        "CND-FRM-045"
    );
}
