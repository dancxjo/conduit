//! Project the exact typed Patchbay graph into renderer-neutral subjects.

use crate::{FaceControlKind, PatchbayGraph, PlanDocument, PlayDocument};
use conduit_presentation::{
    PresentationIconKey, PresentationPropertyValue, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole,
};

use crate::portable_projection::ContentBuilder;

pub(super) fn append_exact_graph(
    form: &str,
    graph: &PatchbayGraph,
    plan: Option<&PlanDocument>,
    play: Option<&PlayDocument>,
    content: &mut ContentBuilder,
) {
    let mut semantic_subjects = Vec::new();
    for composition in &graph.compositions {
        let subject = content.subject_with_identity(
            format!("gear/{}", composition.identity),
            PresentationRole::Gear,
            &composition.gear_name,
            format!(
                "Form-backed Gear {} through checked Back {}",
                composition.gear_name,
                composition.checked_form_id.as_str()
            ),
        );
        let parent = graph
            .compositions
            .iter()
            .filter(|candidate| {
                candidate.identity != composition.identity
                    && composition
                        .gear_name
                        .starts_with(&format!("{}/", candidate.gear_name))
            })
            .max_by_key(|candidate| candidate.gear_name.len())
            .map_or_else(
                || form.to_owned(),
                |candidate| format!("gear/{}", candidate.identity),
            );
        content.contains(&parent, &subject);
        if parent != form {
            identity(content, &subject, "recursive-parent", &parent);
        }
        identity(content, &subject, "semantic-id", &composition.identity);
        identity(
            content,
            &subject,
            "checked-back-id",
            composition.checked_form_id.as_str(),
        );
        identity(content, &subject, "back-source", &composition.back_name);
        text(content, &subject, "reviewed-back", "available");
        text(content, &subject, "back-expanded", "false");
        text(
            content,
            &subject,
            "realization-layer",
            if plan.is_some_and(|plan| {
                plan.exact
                    .realization_backs
                    .iter()
                    .any(|back| back.invocation_path == composition.gear_name)
            }) {
                "recursive"
            } else {
                "not planned"
            },
        );
        for port in composition.inputs.iter().chain(&composition.outputs) {
            let port_subject = content.subject_with_identity(
                format!("port/{}", port.identity),
                PresentationRole::Port,
                port.descriptor.port_id.as_str(),
                format!(
                    "Stable Face {:?} Port {} carrying {}",
                    port.descriptor.direction,
                    port.descriptor.port_id.as_str(),
                    port.descriptor.value_kind.as_str()
                ),
            );
            content.contains(&subject, &port_subject);
            identity(content, &port_subject, "semantic-id", &port.identity);
            text(
                content,
                &port_subject,
                "direction",
                match port.descriptor.direction {
                    conduit_core::PortDirection::Input => "receiving",
                    conduit_core::PortDirection::Output => "outgoing",
                },
            );
            identity(
                content,
                &port_subject,
                "value-kind",
                port.descriptor.value_kind.as_str(),
            );
            text(
                content,
                &port_subject,
                "temporal",
                &format!("{:?}", port.descriptor.temporal),
            );
            semantic_subjects.push((port.identity.as_str(), port_subject));
        }
    }
    for gear in &graph.gears {
        let icon = conduit_semantic_catalog::palette_metadata(&gear.kind_id)
            .map(|metadata| metadata.icon)
            .unwrap_or(PresentationIconKey::GenericGear);
        let subject = content.subject_with_identity(
            format!("gear/{}", gear.identity),
            PresentationRole::Gear,
            gear.gear_id.as_str(),
            format!("{} Gear, {}", gear.gear_id.as_str(), gear.kind_id.as_str()),
        );
        let parent = graph
            .compositions
            .iter()
            .filter(|composition| {
                gear.gear_id
                    .as_str()
                    .starts_with(&format!("{}/", composition.gear_name))
            })
            .max_by_key(|composition| composition.gear_name.len())
            .map_or_else(
                || form.to_owned(),
                |composition| format!("gear/{}", composition.identity),
            );
        content.contains(&parent, &subject);
        if parent != form {
            identity(content, &subject, "recursive-parent", &parent);
        }
        identity(content, &subject, "semantic-id", &gear.identity);
        identity(content, &subject, "kind-id", gear.kind_id.as_str());
        identity(content, &subject, "source-form", &gear.source_form);
        text(content, &subject, "form-path", &gear.form_path.join(" / "));
        text(content, &subject, "icon-token", icon.as_str());
        text(content, &subject, "icon-name", icon.accessibility_name());
        for (index, control) in gear.controls.iter().enumerate() {
            text(
                content,
                &subject,
                &format!("authored-control-{index}"),
                &format!(
                    "{}={} · {}",
                    control.key,
                    control_value(&control.value),
                    control_contract(&control.kind)
                ),
            );
            if let Some(interaction) = &control.interaction {
                identity(
                    content,
                    &subject,
                    &format!("interaction-contract-{index}"),
                    &interaction.contract.contract_identity,
                );
                identity(
                    content,
                    &subject,
                    &format!("interaction-state-{index}"),
                    &interaction.state.state_identity,
                );
                text(
                    content,
                    &subject,
                    &format!("interaction-family-{index}"),
                    interaction_family(&interaction.contract.family),
                );
            }
        }
        append_gear_plan(content, &subject, gear, plan, play);
        semantic_subjects.push((gear.identity.as_str(), subject.clone()));
        for port in gear.inputs.iter().chain(&gear.outputs) {
            let port_subject = content.subject_with_identity(
                format!("port/{}", port.identity),
                PresentationRole::Port,
                port.descriptor.port_id.as_str(),
                format!(
                    "{} {:?} Port carrying {}",
                    port.descriptor.port_id.as_str(),
                    port.descriptor.direction,
                    port.descriptor.value_kind.as_str()
                ),
            );
            content.contains(&subject, &port_subject);
            identity(content, &port_subject, "semantic-id", &port.identity);
            text(
                content,
                &port_subject,
                "direction",
                match port.descriptor.direction {
                    conduit_core::PortDirection::Input => "receiving",
                    conduit_core::PortDirection::Output => "outgoing",
                },
            );
            identity(
                content,
                &port_subject,
                "value-kind",
                port.descriptor.value_kind.as_str(),
            );
            text(
                content,
                &port_subject,
                "temporal",
                &format!("{:?}", port.descriptor.temporal),
            );
            semantic_subjects.push((port.identity.as_str(), port_subject));
        }
    }
    for cord in &graph.cords {
        let subject = content.subject_with_identity(
            format!("cord/{}", cord.identity),
            PresentationRole::Cord,
            "Cord",
            format!("Cord from {} to {}", cord.source_port, cord.sink_port),
        );
        content.contains(form, &subject);
        identity(content, &subject, "semantic-id", &cord.identity);
        identity(content, &subject, "source-port", &cord.source_port);
        identity(content, &subject, "sink-port", &cord.sink_port);
        identity(content, &subject, "value-kind", cord.value_kind.as_str());
        for composition in &graph.compositions {
            if let Some(binding) = composition
                .output_bindings
                .iter()
                .find(|binding| binding.internal_port == cord.source_port)
            {
                identity(
                    content,
                    &subject,
                    "collapsed-source-port",
                    &binding.face_port,
                );
            }
            if let Some(binding) = composition
                .input_bindings
                .iter()
                .find(|binding| binding.internal_port == cord.sink_port)
            {
                identity(content, &subject, "collapsed-sink-port", &binding.face_port);
            }
        }
        append_cord_plan(content, &subject, graph, cord, plan, play);
        for endpoint in [&cord.source_port, &cord.sink_port] {
            if let Some((_, port_subject)) = semantic_subjects
                .iter()
                .find(|(identity, _)| *identity == endpoint)
            {
                content.relationships.push(PresentationRelationship {
                    source: subject.clone(),
                    target: port_subject.clone(),
                    kind: PresentationRelationshipKind::Connects,
                });
            }
        }
    }
}

fn interaction_family(family: &conduit_human::InteractionFamily) -> &'static str {
    match family {
        conduit_human::InteractionFamily::Activate => "Activate",
        conduit_human::InteractionFamily::Boolean => "Boolean",
        conduit_human::InteractionFamily::ChooseOne { .. } => "ChooseOne",
        conduit_human::InteractionFamily::ChooseMany { .. } => "ChooseMany",
        conduit_human::InteractionFamily::Scalar { .. } => "Scalar",
        conduit_human::InteractionFamily::RelativeAdjustment { .. } => "RelativeAdjustment",
        conduit_human::InteractionFamily::Text { .. } => "Text",
        conduit_human::InteractionFamily::Structured { .. } => "Structured",
    }
}

fn append_gear_plan(
    content: &mut ContentBuilder,
    subject: &str,
    gear: &crate::PatchbayGear,
    plan: Option<&PlanDocument>,
    play: Option<&PlayDocument>,
) {
    let Some(plan) = plan else { return };
    let Some(placement) = plan
        .exact
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.gear_id == gear.gear_id)
    else {
        return;
    };
    identity(content, subject, "plan-id", plan.plan_id.as_str());
    text(
        content,
        subject,
        "plan-status",
        if play.is_some() {
            "active"
        } else {
            "candidate"
        },
    );
    text(
        content,
        subject,
        "realization-layer",
        if plan.exact.realization_backs.is_empty() {
            "direct"
        } else {
            "expanded"
        },
    );
    for (name, value) in [
        ("placement-id", placement.placement_id.as_str()),
        ("host-id", placement.host_id.as_str()),
        ("boot-id", placement.boot_id.as_str()),
        ("capability-id", placement.capability_id.as_str()),
        (
            "execution-profile-id",
            placement.execution_profile_id.as_str(),
        ),
        ("implementation-id", placement.implementation_id.as_str()),
        ("artifact-id", placement.artifact_id.as_str()),
    ] {
        identity(content, subject, name, value);
    }
    text(
        content,
        subject,
        "admitted-capacity",
        &format!(
            "active={} queue-items={} queue-bytes={}",
            placement.limits.max_active_instances,
            placement.limits.max_queue_items,
            placement.limits.max_queue_bytes
        ),
    );
    for (index, resource) in placement.resources.iter().enumerate() {
        text(
            content,
            subject,
            &format!("resource-{index}"),
            &format!(
                "{} · class {} · units {}",
                resource.pool_id.as_str(),
                resource.class_id.as_str(),
                resource.units
            ),
        );
    }
    crate::portable_vector_search_projection::append_vector_search_realization(
        content, subject, gear, placement,
    );
    append_play(
        content,
        subject,
        play,
        Some(placement.placement_id.as_str()),
        None,
    );
}

fn append_cord_plan(
    content: &mut ContentBuilder,
    subject: &str,
    graph: &PatchbayGraph,
    cord: &crate::PatchbayCord,
    plan: Option<&PlanDocument>,
    play: Option<&PlayDocument>,
) {
    let Some(plan) = plan else { return };
    let Some(connection) = planned_connection(graph, cord, plan) else {
        return;
    };
    identity(content, subject, "plan-id", plan.plan_id.as_str());
    text(
        content,
        subject,
        "plan-status",
        if play.is_some() {
            "active"
        } else {
            "candidate"
        },
    );
    text(
        content,
        subject,
        "admitted-capacity",
        &format!(
            "items={} bytes={}",
            connection.item_capacity, connection.byte_capacity
        ),
    );
    if let Some(line) = &connection.selected_line {
        identity(content, subject, "line-id", line.line_id.as_str());
        text(
            content,
            subject,
            "base",
            &format!("{:?}", line.binding.base),
        );
        identity(
            content,
            subject,
            "base-instance-id",
            line.binding.base_instance_id.as_str(),
        );
    } else {
        text(content, subject, "line", "local Cord; no external Line");
    }
    append_play(
        content,
        subject,
        play,
        None,
        Some(connection.connection_id.as_str()),
    );
}

fn planned_connection<'a>(
    graph: &PatchbayGraph,
    cord: &crate::PatchbayCord,
    plan: &'a PlanDocument,
) -> Option<&'a conduit_core::PlannedConnection> {
    let source = graph
        .gears
        .iter()
        .flat_map(|gear| &gear.outputs)
        .find(|port| port.identity == cord.source_port)?;
    let sink = graph
        .gears
        .iter()
        .flat_map(|gear| &gear.inputs)
        .find(|port| port.identity == cord.sink_port)?;
    let source_placement = plan
        .exact
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.gear_id == source.gear_id)?;
    let sink_placement = plan
        .exact
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.gear_id == sink.gear_id)?;
    plan.exact
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|connection| {
            connection.source_placement_id == source_placement.placement_id
                && connection.sink_placement_id == sink_placement.placement_id
                && connection.source_port_id == source.descriptor.port_id
                && connection.sink_port_id == sink.descriptor.port_id
        })
}

fn append_play(
    content: &mut ContentBuilder,
    subject: &str,
    play: Option<&PlayDocument>,
    placement: Option<&str>,
    connection: Option<&str>,
) {
    let Some(play) = play else { return };
    identity(
        content,
        subject,
        "active-play-id",
        play.active_play_id.as_str(),
    );
    text(
        content,
        subject,
        "play-state",
        &format!("{:?}", play.terminal),
    );
    text(content, subject, "pressure", "not exposed by this Play");
    for (index, sign) in play
        .signs
        .iter()
        .filter(|sign| {
            placement.is_some_and(|value| {
                sign.placement_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == value)
            }) || connection.is_some_and(|value| {
                sign.connection_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == value)
            })
        })
        .enumerate()
    {
        text(
            content,
            subject,
            &format!("sign-{index}"),
            &format!("{} · {:?}", sign.sign_id.as_str(), sign.kind),
        );
    }
}

fn identity(content: &mut ContentBuilder, subject: &str, name: &str, value: &str) {
    content.property(
        subject,
        name,
        PresentationPropertyValue::Identity(value.into()),
    );
}

fn text(content: &mut ContentBuilder, subject: &str, name: &str, value: &str) {
    content.property(subject, name, PresentationPropertyValue::Text(value.into()));
}

fn control_value(value: &conduit_core::ConfigurationValue) -> String {
    match value {
        conduit_core::ConfigurationValue::Bool(value) => value.to_string(),
        conduit_core::ConfigurationValue::U64(value) => value.to_string(),
        conduit_core::ConfigurationValue::I64(value) => value.to_string(),
        conduit_core::ConfigurationValue::Text(value) => format!("\"{value}\""),
        conduit_core::ConfigurationValue::Structured(value) => format!(
            "<structured:{}:{}-bytes>",
            value.profile().as_str(),
            value.canonical_value().len()
        ),
    }
}

fn control_contract(kind: &FaceControlKind) -> String {
    match kind {
        FaceControlKind::BooleanChoice { choices } => choices.join("|"),
        FaceControlKind::TextChoice { choices } => choices.join("|"),
        FaceControlKind::Number {
            minimum,
            maximum,
            unit,
        }
        | FaceControlKind::Range {
            minimum,
            maximum,
            unit,
        } => {
            format!("{minimum}..{maximum}{}", unit.unwrap_or(""))
        }
        FaceControlKind::ScalarNumber {
            minimum,
            maximum,
            unit,
        } => {
            format!("{minimum}..{maximum}{unit}")
        }
        FaceControlKind::ShortText { maximum_bytes } => format!("max {maximum_bytes} bytes"),
    }
}
