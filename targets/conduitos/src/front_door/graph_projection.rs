//! Project the selected admitted Plot graph into the same exact native Face.
//! Geometry remains the graphical Mask's decision.

use alloc::{format, string::String};
use conduit_presentation::{
    PresentationDisclosure, PresentationDisclosureLevel, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};
use patchbay_graph::{PatchbayFrontPort, PatchbayGraph};

pub(super) struct Content<'a> {
    pub subjects: &'a mut alloc::vec::Vec<PresentationSubject>,
    pub relationships: &'a mut alloc::vec::Vec<PresentationRelationship>,
    pub properties: &'a mut alloc::vec::Vec<PresentationProperty>,
    pub texts: &'a mut alloc::vec::Vec<PresentationText>,
    pub disclosures: &'a mut alloc::vec::Vec<PresentationDisclosure>,
}

pub(super) fn append(graph: &PatchbayGraph, body: &str, content: &mut Content<'_>) {
    let plot = format!("plot/{}", graph.checked_plot_id.as_str());
    let new_plot = !content
        .subjects
        .iter()
        .any(|subject| subject.identity == plot);
    if new_plot {
        subject(content, &plot, PresentationRole::Plot, &graph.plot_name);
    }
    relation(content, body, &plot, PresentationRelationshipKind::Contains);
    property(
        content,
        &plot,
        "source-document-id",
        graph.source_document_id.as_str(),
    );
    property(
        content,
        &plot,
        "expanded-plot-id",
        graph.expanded_plot_id.as_str(),
    );
    if new_plot {
        content.disclosures.push(PresentationDisclosure {
            subject: plot.clone(),
            level: PresentationDisclosureLevel::Context,
        });
    }
    content.texts.push(PresentationText {
        subject: plot.clone(),
        text: "Exact selected Plot graph; connections are typed Ports and Cords".into(),
    });
    for gear in &graph.gears {
        let id = format!("gear/{}", gear.identity);
        subject(
            content,
            &id,
            PresentationRole::Gear,
            &format!("{} Gear · {}", gear.gear_id.as_str(), gear.kind_id.as_str()),
        );
        relation(content, &plot, &id, PresentationRelationshipKind::Contains);
        property(content, &id, "semantic-id", &gear.identity);
        property(content, &id, "kind-id", gear.kind_id.as_str());
        for port in gear.inputs.iter().chain(&gear.outputs) {
            append_port(content, &id, &port.identity, &port.descriptor);
        }
    }
    for composition in &graph.compositions {
        let id = format!("gear/{}", composition.identity);
        subject(
            content,
            &id,
            PresentationRole::Gear,
            &format!("{} · checked Plot Back", composition.gear_name),
        );
        relation(content, &plot, &id, PresentationRelationshipKind::Contains);
        property(content, &id, "semantic-id", &composition.identity);
        property(
            content,
            &id,
            "checked-back-id",
            composition.checked_plot_id.as_str(),
        );
        for port in composition.inputs.iter().chain(&composition.outputs) {
            append_front_port(content, &id, port);
        }
    }
    for port in graph.front_inputs.iter().chain(&graph.front_outputs) {
        append_front_port(content, &plot, port);
    }
    for cord in &graph.cords {
        let id = format!("cord/{}", cord.identity);
        subject(content, &id, PresentationRole::Cord, "Typed connection");
        relation(content, &plot, &id, PresentationRelationshipKind::Contains);
        property(content, &id, "semantic-id", &cord.identity);
        property(content, &id, "source-port", &cord.source_port);
        property(content, &id, "sink-port", &cord.sink_port);
        property(content, &id, "value-kind", cord.value_kind.as_str());
        for endpoint in [&cord.source_port, &cord.sink_port] {
            relation(
                content,
                &id,
                &format!("port/{endpoint}"),
                PresentationRelationshipKind::Connects,
            );
        }
    }
}

fn append_front_port(content: &mut Content<'_>, parent: &str, port: &PatchbayFrontPort) {
    append_port(content, parent, &port.identity, &port.descriptor);
}

fn append_port(
    content: &mut Content<'_>,
    parent: &str,
    identity: &str,
    descriptor: &conduit_core::PortDescriptor,
) {
    let id = format!("port/{identity}");
    subject(
        content,
        &id,
        PresentationRole::Port,
        &format!(
            "{} · {}",
            descriptor.port_id.as_str(),
            descriptor.value_kind.as_str()
        ),
    );
    relation(content, parent, &id, PresentationRelationshipKind::Contains);
    property(content, &id, "semantic-id", identity);
    property(content, &id, "value-kind", descriptor.value_kind.as_str());
    content.properties.push(PresentationProperty {
        subject: id,
        name: "direction".into(),
        value: PresentationPropertyValue::Text(
            match descriptor.direction {
                conduit_core::PortDirection::Input => "receiving",
                conduit_core::PortDirection::Output => "outgoing",
            }
            .into(),
        ),
    });
}

fn subject(content: &mut Content<'_>, identity: &str, role: PresentationRole, name: &str) {
    content.subjects.push(PresentationSubject {
        identity: identity.into(),
        role,
        name: name.into(),
    });
}
fn relation(
    content: &mut Content<'_>,
    source: &str,
    target: &str,
    kind: PresentationRelationshipKind,
) {
    content.relationships.push(PresentationRelationship {
        source: source.into(),
        target: target.into(),
        kind,
    });
}
fn property(content: &mut Content<'_>, subject: &str, name: &str, value: &str) {
    content.properties.push(PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value: PresentationPropertyValue::Identity(String::from(value)),
    });
}
