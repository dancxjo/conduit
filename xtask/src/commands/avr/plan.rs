//! Ordinary Conduit planning for the AVR contact observation.

use conduit_core::{
    bind_active_play, encode_assigned_activation, AssignedActivation, AssignedIdentity,
    AssignedPlanMaxima, BaseImplementationId, BootId, HostId, OfferGeneration,
    ASSIGNED_ACTIVATION_BYTES,
};
use conduit_embedded_build::{encode_assigned_plan, generate_embedded_plan, EmbeddedImageBounds};
use conduit_pete::{
    catalogs, live_create_observation_advertisement, CreateObservationEvidence, OiMode,
};
use conduit_plan_lowering::lowering::lower_plan_fragment;

pub(super) const AVR_HOST_ID: &str = "host/avr-promicro/create1";
const CONTACT_FORM: &str = "form contact_sample {\n contact: robotics/observe-contact\n}\n";

pub(super) struct PlannedContact {
    pub(super) assigned: Vec<u8>,
    pub(super) activation: [u8; ASSIGNED_ACTIVATION_BYTES],
    pub(super) expected_output_port: u16,
    pub(super) expected_value_bytes: usize,
    pub(super) plan: AssignedIdentity,
    pub(super) fragment: AssignedIdentity,
    pub(super) host: AssignedIdentity,
    pub(super) boot: AssignedIdentity,
    pub(super) active_play: AssignedIdentity,
}

pub(super) fn plan_contact(boot_id: &str) -> Result<PlannedContact, Box<dyn std::error::Error>> {
    if !boot_id.starts_with("avr-") || boot_id.len() != 12 {
        return Err("AVR planning requires the observed avr-XXXXXXXX Boot identity".into());
    }
    let (startup, profile) = catalogs()?;
    let syntax = conduit_form::parse_syntax_document(CONTACT_FORM);
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("AVR contact Form check refused: {error:?}"))?;
    let expanded = conduit_form::expand_canonical_form(&checked, "contact_sample", &profile)?;
    let evidence = CreateObservationEvidence {
        host_id: HostId::from(AVR_HOST_ID),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        serial_base_id: "base/avr-promicro/create1-uart".into(),
        robot_identity: "create/attached-1".into(),
        session_resource_id: "session/avr-promicro/create1".into(),
        mode: OiMode::Full,
        observed_at_tick: 0,
        maximum_age_ticks: 1,
    };
    let host = live_create_observation_advertisement(&evidence, 0)
        .map_err(|error| format!("AVR Host offer refused: {error:?}"))?;
    let placements =
        conduit_planner::default_expanded_placements(&expanded, core::slice::from_ref(&host))?;
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )?;
    if plan.fragments.len() != 1 {
        return Err("AVR contact Form did not plan to one exact fragment".into());
    }
    let fragment = &plan.fragments[0];
    let lowered = lower_plan_fragment(fragment)
        .map_err(|error| format!("AVR assigned fragment lowering refused: {error:?}"))?;
    let generated = generate_embedded_plan(fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING)?;
    let assigned = encode_assigned_plan(&generated, AssignedPlanMaxima::SINGLE_SOURCE)?;
    if generated.output_ports.len() != 1 || generated.host_operations.len() != 1 {
        return Err("AVR contact fragment is not the admitted one-source shape".into());
    }
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let activation = AssignedActivation {
        plan: AssignedIdentity::from_text(plan.plan_id.as_str()),
        fragment: AssignedIdentity::from_text(fragment.fragment_id.as_str()),
        host: AssignedIdentity::from_text(fragment.host_id.as_str()),
        boot: AssignedIdentity::from_text(fragment.boot_id.as_str()),
        active_play: AssignedIdentity::from_text(active.active_play_id.as_str()),
    };
    Ok(PlannedContact {
        assigned,
        activation: encode_assigned_activation(activation),
        expected_output_port: generated.output_ports[0].port,
        expected_value_bytes: generated.host_operations[0].maximum_output_bytes as usize,
        plan: activation.plan,
        fragment: activation.fragment,
        host: activation.host,
        boot: activation.boot,
        active_play: activation.active_play,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{decode_assigned_activation, decode_assigned_single_source};

    #[test]
    fn ordinary_form_plans_to_the_exact_generic_avr_projection() {
        for forbidden in ["avr", "create", "uart", "host", "boot", "gpio"] {
            assert!(!CONTACT_FORM.contains(forbidden));
        }
        let planned = plan_contact("avr-00000001").unwrap();
        assert!(planned.assigned.len() <= 544);
        assert_eq!(planned.expected_output_port, 0);
        assert_eq!(planned.expected_value_bytes, 1);
        let active = decode_assigned_activation(&planned.activation).unwrap();
        assert_eq!(active.plan, planned.plan);
        assert_eq!(active.fragment, planned.fragment);
        assert_eq!(active.host, planned.host);
        assert_eq!(active.boot, planned.boot);
        assert_eq!(active.active_play, planned.active_play);
        let decoded = decode_assigned_single_source(
            &planned.assigned,
            AssignedPlanMaxima::SINGLE_SOURCE,
            conduit_core::AssignedSingleSourceRequirements {
                host: planned.host,
                boot: planned.boot,
                counts: [1, 1, 0, 0, 0, 0, 1, 3, 4, 0, 1, 2],
                operation: AssignedIdentity::from_text("pete.host/create1-observe-contact@1"),
                resources: &[0, 1, 2],
            },
        )
        .unwrap();
        assert_eq!(decoded.output_port, planned.expected_output_port);
    }
}
