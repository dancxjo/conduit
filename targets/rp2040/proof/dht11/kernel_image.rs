//! Build-only inert bench template, not a physical availability advertisement.
#[allow(dead_code, reason = "shared fixture retains raw-capture entry points")]
#[path = "../../../../bodies/environment/runtime/tests/support/plan.rs"]
mod plan;
mod pulse {
    use conduit_core::*;
    use conduit_pulse_capture::{installation::*, owner::*, *};
    pub struct Provider;
    impl PulseReadProvider for Provider {
        async fn read(
            &mut self,
            _: CheckedPulseRead,
        ) -> Result<PulseReadCapture, PulseReadRefusal> {
            unreachable!("inert Make model")
        }
    }
    pub fn identity() -> PulseNativeIdentity {
        PulseNativeIdentity {
            host_id: plan_host(),
            boot_id: "fixture/environment/boot".into(),
            base_id: "fixture/pulse-base".into(),
            provider_instance_id: "fixture/pulse-provider".into(),
            provider_generation: 1,
            resource_pool_id: "fixture/pulse-resource".into(),
            resource_generation_id: ResourceGenerationId("fixture/attachment/1".into()),
            envelope_id: "fixture/pulse-envelope".into(),
            artifact_id: "fixture/pulse-artifact".into(),
            implementation_id: "fixture/pulse-read@1".into(),
            base_implementation_id: "fixture/pulse-base@1".into(),
        }
    }
    fn plan_host() -> HostId {
        super::plan::PREFIX.into()
    }
    pub fn grant() -> AuthorityGrant {
        AuthorityGrant {
            grant_id: "fixture/pulse-grant".into(),
            contract_id: PULSE_AUTHORITY.into(),
            host_call_contract_id: PULSE_READ_CALL.into(),
            subject_kind: PULSE_READ_KIND.into(),
            host_id: identity().host_id,
            boot_id: identity().boot_id,
            capability_id: PULSE_CAPABILITY.into(),
        }
    }
}
pub fn prepare(out: &std::path::Path) {
    use conduit_pulse_capture::{installation::*, owner::*};
    let attachment = PulseAttachment {
        generation: 1,
        resource_bytes: 64,
        limits: conduit_pulse_capture::PulseReadLimits {
            maximum_hold_low_us: 50000,
            maximum_skipped_pulses: 4,
            minimum_sample_offset_us: 10,
            maximum_sample_offset_us: 200,
            maximum_octets: 8,
            maximum_capture_timeout_us: 50000,
        },
    };
    // Inert bench realization used only as construction material for the
    // freestanding test. Runtime must own the real resources and reseal a new Boot.
    let ready =
        unsafe { ReadyPulseBase::new(pulse::identity(), attachment, pulse::Provider) }.unwrap();
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let (model, lowered) = plan::prepare_with_cadence(
        &ready,
        contract.kind().clone(),
        conduit_time::tick_semantic_contract(),
        if std::env::var_os("CARGO_FEATURE_HTTP_AP").is_some() {
            4096
        } else {
            4
        },
        if std::env::var_os("CARGO_FEATURE_HTTP_AP").is_some() {
            15000
        } else {
            3000
        },
    );
    // The rich Plan uses omitted default fields; a positional format cannot
    // round-trip it. This trusted bench construction template uses normal JSON.
    let encoded = serde_json::to_vec(&model).unwrap();
    let decoded: conduit_core::Plan = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(
        model, decoded,
        "Make must round-trip its exact inert template"
    );
    std::fs::write(out.join("kernel-template.json"), encoded).unwrap();
    let image = conduit_embedded_build::generate_embedded_plan_with_computations(
        &model.fragments[0],
        &lowered,
        conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
    )
    .unwrap();
    let mut code = String::from("const ROLES: &[Role] = &[\n");
    for node in &image.nodes {
        if node.kind_id == format!("{}/begin", plan::PREFIX) {
            code.push_str("Role::Begin,\n");
            continue;
        }
        if let Some(index) = [
            "decoded",
            "timed-out",
            "provider-failed",
            "geometry-refused",
        ]
        .iter()
        .position(|name| node.kind_id == format!("{}/{name}", plan::PREFIX))
        {
            code.push_str(&format!("Role::Observe({index}),\n"));
            continue;
        }
        if node.kind_id == conduit_pulse_capture::PULSE_READ_KIND {
            code.push_str("Role::Pulse,\n");
            continue;
        }
        if node.kind_id == conduit_time::TICK_TRIGGER_KIND {
            code.push_str("Role::Trigger,\n");
            continue;
        }
        if node.kind_id == conduit_time::TICK_KIND {
            let value = |key: &str| {
                let config = image
                    .configuration
                    .iter()
                    .find(|c| c.node == node.node && c.key == key)
                    .unwrap();
                let conduit_embedded_build::GeneratedConfigurationValue::U64(value) = config.value
                else {
                    panic!("tick configuration")
                };
                value
            };
            code.push_str(&format!(
                "Role::Tick({}, {}),\n",
                value("count"),
                value("period-ms")
            ));
            continue;
        }
        if node.kind_id == conduit_environment_runtime::observation::KIND {
            code.push_str("Role::Observation,\n");
            continue;
        }
        let config = image
            .configuration
            .iter()
            .find(|c| c.node == node.node)
            .unwrap();
        let conduit_embedded_build::GeneratedConfigurationValue::Text(text) = &config.value else {
            panic!("code")
        };
        if config.key == "program" {
            let program =
                conduit_plot::PortableExpressionProgram::from_canonical_hex(text).unwrap();
            let path = format!("node-{}.program", node.node);
            std::fs::write(out.join(&path), program.canonical_bytes().unwrap()).unwrap();
            code.push_str(&format!(
                "Role::Expression(include_bytes!(concat!(env!(\"OUT_DIR\"),\"/{path}\"))),\n"
            ));
        } else {
            code.push_str(&format!("Role::Selector({text:?}),\n"));
        }
    }
    code.push_str("];\n");
    std::fs::write(out.join("kernel-roles.rs"), code).unwrap();
}
