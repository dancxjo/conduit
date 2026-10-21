//! Append exact admitted Plot graph meaning to an existing Face composition.
//! Shared by Masks; geometry remains a realization decision.
//! The caller validates and admits the completed Face before presentation.

use alloc::{format, string::String};
use conduit_face::{
    FaceDisclosure, FaceDisclosureLevel, FaceProperty, FacePropertyValue, FaceRelationship,
    FaceRelationshipKind, FaceRole, FaceSubject, FaceText,
};
use patchbay_graph::{PatchbayCord, PatchbayFrontPort, PatchbayGraph};

pub struct Content<'a> {
    pub subjects: &'a mut alloc::vec::Vec<FaceSubject>,
    pub relationships: &'a mut alloc::vec::Vec<FaceRelationship>,
    pub properties: &'a mut alloc::vec::Vec<FaceProperty>,
    pub texts: &'a mut alloc::vec::Vec<FaceText>,
    pub disclosures: &'a mut alloc::vec::Vec<FaceDisclosure>,
}

pub fn append(graph: &PatchbayGraph, body: &str, content: &mut Content<'_>) {
    let plot = format!("plot/{}", graph.checked_plot_id.as_str());
    let new_plot = !content
        .subjects
        .iter()
        .any(|subject| subject.identity == plot);
    if new_plot {
        subject(content, &plot, FaceRole::Plot, &graph.plot_name);
    }
    relation(content, body, &plot, FaceRelationshipKind::Contains);
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
        content.disclosures.push(FaceDisclosure {
            subject: plot.clone(),
            level: FaceDisclosureLevel::Context,
        });
    }
    content.texts.push(FaceText {
        subject: plot.clone(),
        text: "Exact selected Plot graph; connections are typed Ports and Cords".into(),
    });
    for gear in &graph.gears {
        let id = format!("gear/{}", gear.identity);
        subject(
            content,
            &id,
            FaceRole::Gear,
            &format!("{} Gear · {}", gear.gear_id.as_str(), gear.kind_id.as_str()),
        );
        relation(content, &plot, &id, FaceRelationshipKind::Contains);
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
            FaceRole::Gear,
            &format!("{} · checked Plot Back", composition.gear_name),
        );
        relation(content, &plot, &id, FaceRelationshipKind::Contains);
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
        append_named_cord(
            &mut GraphFacts {
                subjects: content.subjects,
                relationships: content.relationships,
                properties: content.properties,
            },
            &plot,
            cord,
            "Typed connection",
        );
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
    append_named_port(
        &mut GraphFacts {
            subjects: content.subjects,
            relationships: content.relationships,
            properties: content.properties,
        },
        parent,
        identity,
        descriptor,
        &format!(
            "{} · {}",
            descriptor.port_id.as_str(),
            descriptor.value_kind.as_str()
        ),
    );
}

/// Collections changed by exact graph fact projection; no rendering state.
pub struct GraphFacts<'a> {
    pub subjects: &'a mut alloc::vec::Vec<FaceSubject>,
    pub relationships: &'a mut alloc::vec::Vec<FaceRelationship>,
    pub properties: &'a mut alloc::vec::Vec<FaceProperty>,
}

/// Shared exact Port projection with caller-owned human wording.
/// The completed graph/Face remains subject to ordinary validation/admission.
pub fn append_named_port(
    content: &mut GraphFacts<'_>,
    parent: &str,
    identity: &str,
    descriptor: &conduit_core::PortDescriptor,
    name: &str,
) -> String {
    let id = format!("port/{identity}");
    if let Some(existing) = content
        .subjects
        .iter()
        .find(|subject| subject.identity == id)
    {
        debug_assert_eq!(existing.role, FaceRole::Port);
    } else {
        content.subjects.push(FaceSubject {
            identity: id.clone(),
            role: FaceRole::Port,
            name: name.into(),
        });
    }
    content.relationships.push(FaceRelationship {
        source: parent.into(),
        target: id.clone(),
        kind: FaceRelationshipKind::Contains,
    });
    let mut property = |name: &str, value| {
        content.properties.push(FaceProperty {
            subject: id.clone(),
            name: name.into(),
            value,
        })
    };
    property("semantic-id", FacePropertyValue::Identity(identity.into()));
    property(
        "port-id",
        FacePropertyValue::Identity(descriptor.port_id.as_str().into()),
    );
    property(
        "value-kind",
        FacePropertyValue::Identity(descriptor.value_kind.as_str().into()),
    );
    property(
        "temporal",
        FacePropertyValue::Text(descriptor.temporal.as_str().into()),
    );
    if let Some(kind) = &descriptor.abnormal_kind {
        property(
            "abnormal-kind",
            FacePropertyValue::Identity(kind.as_str().into()),
        );
    }
    property(
        "direction",
        FacePropertyValue::Text(
            match descriptor.direction {
                conduit_core::PortDirection::Input => "receiving",
                conduit_core::PortDirection::Output => "outgoing",
            }
            .into(),
        ),
    );
    id
}

fn subject(content: &mut Content<'_>, identity: &str, role: FaceRole, name: &str) {
    content.subjects.push(FaceSubject {
        identity: identity.into(),
        role,
        name: name.into(),
    });
}
fn relation(content: &mut Content<'_>, source: &str, target: &str, kind: FaceRelationshipKind) {
    content.relationships.push(FaceRelationship {
        source: source.into(),
        target: target.into(),
        kind,
    });
}
fn property(content: &mut Content<'_>, subject: &str, name: &str, value: &str) {
    content.properties.push(FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value: FacePropertyValue::Identity(String::from(value)),
    });
}

/// Preserve exact ordered endpoints even when an endpoint is absent. Only real
/// Port subjects receive Connects relationships; no placeholder is fabricated.
pub fn append_named_cord(
    content: &mut GraphFacts<'_>,
    parent: &str,
    cord: &PatchbayCord,
    name: &str,
) -> String {
    let id = format!("cord/{}", cord.identity);
    if let Some(existing) = content
        .subjects
        .iter()
        .find(|subject| subject.identity == id)
    {
        debug_assert_eq!(existing.role, FaceRole::Cord);
    } else {
        content.subjects.push(FaceSubject {
            identity: id.clone(),
            role: FaceRole::Cord,
            name: name.into(),
        });
    }
    content.relationships.push(FaceRelationship {
        source: parent.into(),
        target: id.clone(),
        kind: FaceRelationshipKind::Contains,
    });
    for (name, value) in [
        ("semantic-id", cord.identity.as_str()),
        ("source-port", cord.source_port.as_str()),
        ("sink-port", cord.sink_port.as_str()),
        ("value-kind", cord.value_kind.as_str()),
    ] {
        content.properties.push(FaceProperty {
            subject: id.clone(),
            name: name.into(),
            value: FacePropertyValue::Identity(value.into()),
        });
    }
    content.properties.push(FaceProperty {
        subject: id.clone(),
        name: "temporal".into(),
        value: FacePropertyValue::Text(cord.temporal.as_str().into()),
    });
    for endpoint in [&cord.source_port, &cord.sink_port] {
        let target = format!("port/{endpoint}");
        if content
            .subjects
            .iter()
            .any(|subject| subject.identity == target && subject.role == FaceRole::Port)
        {
            content.relationships.push(FaceRelationship {
                source: id.clone(),
                target,
                kind: FaceRelationshipKind::Connects,
            });
        }
    }
    id
}
