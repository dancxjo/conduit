//! Project the exact typed Patchbay graph into renderer-neutral subjects.

use crate::{FaceControlKind, PatchbayGraph};
use conduit_presentation::{
    PresentationIconKey, PresentationPropertyValue, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole,
};

use crate::portable_projection::ContentBuilder;

pub(super) fn append_exact_graph(form: &str, graph: &PatchbayGraph, content: &mut ContentBuilder) {
    let mut semantic_subjects = Vec::new();
    for gear in &graph.gears {
        let icon = conduit_std_catalog::palette_metadata(&gear.kind_id)
            .map(|metadata| metadata.icon)
            .unwrap_or(PresentationIconKey::GenericGear);
        let subject = content.subject(
            PresentationRole::Gear,
            gear.gear_id.as_str(),
            format!("{} Gear, {}", gear.gear_id.as_str(), gear.kind_id.as_str()),
        );
        content.contains(form, &subject);
        identity(content, &subject, "semantic-id", &gear.identity);
        identity(content, &subject, "kind-id", gear.kind_id.as_str());
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
        }
        semantic_subjects.push((gear.identity.as_str(), subject.clone()));
        for port in gear.inputs.iter().chain(&gear.outputs) {
            let port_subject = content.subject(
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
        let subject = content.subject(
            PresentationRole::Cord,
            "Cord",
            format!("Cord from {} to {}", cord.source_port, cord.sink_port),
        );
        content.contains(form, &subject);
        identity(content, &subject, "semantic-id", &cord.identity);
        identity(content, &subject, "source-port", &cord.source_port);
        identity(content, &subject, "sink-port", &cord.sink_port);
        identity(content, &subject, "value-kind", cord.value_kind.as_str());
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
