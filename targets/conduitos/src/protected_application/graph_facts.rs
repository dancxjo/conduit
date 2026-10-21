//! Root checks scope and object membership, never recomputes the guest projector.
use super::*;
use alloc::format;
use conduit_face::{FacePropertyValue, FaceRelationshipKind, FaceRole};
use conduitos_application_core::graph_facts::{Contribution, Scope};
fn query(
    service: &mut Service,
    graph: &PatchbayGraph,
    scope: Scope,
) -> Result<Contribution, Refusal> {
    scope.validate(graph)?;
    let prepared = service.command(&Command::PrepareGraphProjection {
        graph: graph.clone(),
        scope: scope.clone(),
    })?;
    if !matches!(prepared, Reply::Prepared) {
        return Err(Refusal::Malformed);
    }
    let reply = service.command(&Command::FinishGraphProjection)?;
    let Reply::GraphFacts { contribution } = reply else {
        return Err(Refusal::Malformed);
    };
    validate(&contribution, graph, &scope)?;
    Ok(contribution)
}
pub(crate) fn project_graph(graph: &PatchbayGraph, scope: Scope) -> Result<Contribution, Refusal> {
    let mut service = Service::install()?;
    let result = query(&mut service, graph, scope);
    service.quarantine();
    result
}
impl Owner {
    pub fn project_graph(
        &self,
        graph: &PatchbayGraph,
        scope: Scope,
    ) -> Result<Contribution, Refusal> {
        if self.failed.get() {
            return Err(Refusal::Order);
        }
        let result = if self.needed {
            (|| {
                let mut slot = self.service.try_borrow_mut().map_err(|_| Refusal::Order)?;
                query(slot.as_mut().ok_or(Refusal::Order)?, graph, scope)
            })()
        } else {
            project_graph(graph, scope)
        };
        if result.is_err() {
            self.failed.set(true);
            if let Ok(mut slot) = self.service.try_borrow_mut() {
                if let Some(mut service) = slot.take() {
                    service.quarantine();
                }
            }
        }
        result
    }
}
fn role(graph: &PatchbayGraph, id: &str) -> Option<FaceRole> {
    if id == format!("plot/{}", graph.checked_plot_id.as_str()) {
        return Some(FaceRole::Plot);
    }
    if let Some(raw) = id.strip_prefix("gear/") {
        if graph.gears.iter().any(|gear| gear.identity == raw)
            || graph.compositions.iter().any(|gear| gear.identity == raw)
        {
            return Some(FaceRole::Gear);
        }
    }
    if let Some(raw) = id.strip_prefix("port/") {
        if graph
            .front_inputs
            .iter()
            .chain(&graph.front_outputs)
            .any(|port| port.identity == raw)
            || graph.gears.iter().any(|gear| {
                gear.inputs
                    .iter()
                    .chain(&gear.outputs)
                    .any(|port| port.identity == raw)
            })
            || graph.compositions.iter().any(|gear| {
                gear.inputs
                    .iter()
                    .chain(&gear.outputs)
                    .any(|port| port.identity == raw)
            })
        {
            return Some(FaceRole::Port);
        }
    }
    if let Some(raw) = id.strip_prefix("cord/") {
        if graph.cords.iter().any(|cord| cord.identity == raw) {
            return Some(FaceRole::Cord);
        }
    }
    None
}
fn validate(result: &Contribution, graph: &PatchbayGraph, scope: &Scope) -> Result<(), Refusal> {
    use conduit_face::{
        MAX_FACE_PROPERTIES, MAX_FACE_RELATIONSHIPS, MAX_FACE_SUBJECTS, MAX_FACE_TEXT_ITEMS,
    };
    if &result.scope != scope
        || result.subjects.len() > MAX_FACE_SUBJECTS
        || result.relationships.len() > MAX_FACE_RELATIONSHIPS
        || result.properties.len() > MAX_FACE_PROPERTIES
        || result.text.len() > MAX_FACE_TEXT_ITEMS
        || result.disclosures.len() > MAX_FACE_SUBJECTS
    {
        return Err(Refusal::Malformed);
    }
    let plot = format!("plot/{}", graph.checked_plot_id.as_str());
    let body = format!("body/{}", scope.body.as_str());
    if result.subjects.iter().any(|subject| {
        role(graph, &subject.identity) != Some(subject.role.clone())
            || (scope.plot_present && subject.identity == plot)
    }) || (!scope.plot_present
        && !result
            .subjects
            .iter()
            .any(|subject| subject.identity == plot))
    {
        return Err(Refusal::Malformed);
    }
    let mut owner_edge = false;
    for relation in &result.relationships {
        if relation.source == body {
            if relation.kind != FaceRelationshipKind::Contains || relation.target != plot {
                return Err(Refusal::Malformed);
            }
            owner_edge = true;
        } else if role(graph, &relation.source).is_none()
            || role(graph, &relation.target).is_none()
            || !matches!(
                relation.kind,
                FaceRelationshipKind::Contains | FaceRelationshipKind::Connects
            )
        {
            return Err(Refusal::Malformed);
        }
    }
    if !owner_edge
        || result
            .properties
            .iter()
            .any(|property| role(graph, &property.subject).is_none())
        || result
            .text
            .iter()
            .any(|text| role(graph, &text.subject).is_none())
        || result
            .disclosures
            .iter()
            .any(|entry| role(graph, &entry.subject).is_none())
    {
        return Err(Refusal::Malformed);
    }
    for (name, expected) in [
        ("source-document-id", graph.source_document_id.as_str()),
        ("expanded-plot-id", graph.expanded_plot_id.as_str()),
    ] {
        let mut entries = result
            .properties
            .iter()
            .filter(|property| property.subject == plot && property.name == name);
        let property = entries.next().ok_or(Refusal::Malformed)?;
        if entries.next().is_some()
            || property.value != FacePropertyValue::Identity(expected.into())
        {
            return Err(Refusal::Malformed);
        }
    }
    for property in &result.properties {
        if property.name == "semantic-id" {
            let (_, raw) = property.subject.split_once('/').ok_or(Refusal::Malformed)?;
            if property.value != FacePropertyValue::Identity(raw.into()) {
                return Err(Refusal::Malformed);
            }
        }
    }
    // Validate fragment strings/duplicates/references with canonical Face laws
    // before caching. These synthetic owner subjects are validation context only;
    // this Face is never returned, shown, or used as a semantic owner identity.
    let mut subjects = result.subjects.clone();
    subjects.push(conduit_face::FaceSubject {
        identity: body,
        name: "Owner".into(),
        role: FaceRole::Body,
    });
    if scope.plot_present {
        subjects.push(conduit_face::FaceSubject {
            identity: plot,
            name: "Plot".into(),
            role: FaceRole::Plot,
        });
    }
    conduit_face::Face::new_with_semantics(
        scope.owner_revision,
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: alloc::vec::Vec::new(),
        },
        subjects,
        result.relationships.clone(),
        result.properties.clone(),
        result.text.clone(),
        alloc::vec::Vec::new(),
        result.disclosures.clone(),
    )
    .map_err(|_| Refusal::Malformed)?;
    Ok(())
}
