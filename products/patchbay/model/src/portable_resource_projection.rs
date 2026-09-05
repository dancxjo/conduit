//! Resource residence disclosure over sealed Plan facts, separate from Cord/Line.
use crate::portable_projection::ContentBuilder;
use conduit_core::PlannedGear;
use conduit_presentation::PresentationPropertyValue;

pub(super) fn append_resources(
    content: &mut ContentBuilder,
    subject: &str,
    placement: &PlannedGear,
) {
    for (index, resource) in placement.resources.iter().enumerate() {
        content.property(
            subject,
            &format!("resource-{index}"),
            PresentationPropertyValue::Text(format!(
                "{} · class {} · units {}",
                resource.pool_id.as_str(),
                resource.class_id.as_str(),
                resource.units
            )),
        );
        let Some(residence) = &resource.content else {
            continue;
        };
        let c = &residence.contract;
        let identity = c
            .identity
            .digest()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let version = c
            .version
            .digest()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        for (key,value) in [
            ("meaning",format!("RESOURCE {} · identity {identity} · generation {version}",c.content_profile.as_str())),
            ("access",format!("{:?} · {:?} · {:?} · sensitive {}",c.access,c.sharing,c.retention,c.sensitive)),
            ("bounds",format!("{} bytes · {} items · {} generations · {} reader leases · {} publication slots",c.maximum_bytes,c.maximum_items,c.generation_slots,c.reader_leases,c.publication_slots)),
            ("residence",format!("owner {} · Boot {} · residence {} · Base {}",residence.owner_host.as_str(),residence.owner_boot.as_str(),residence.residence_profile.as_str(),residence.base_id.as_str())),
        ] { content.property(subject,&format!("resource-{index}-{key}"),PresentationPropertyValue::Text(value)); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_planner::proof::resource_frame::frame_resource_plan;
    #[test]
    fn resource_generation_and_residence_descend_separately_from_cord_line() {
        let proof = frame_resource_plan(false, false).unwrap();
        let before = serde_json::to_vec(&proof.plan).unwrap();
        let graph = crate::PatchbayGraph::from_expanded(&proof.expanded).unwrap();
        let document = crate::PlanDocument::from_plan(
            crate::PatchbayRequestId::new("resource-inspect").unwrap(),
            &proof.plan,
        )
        .unwrap();
        let mut content = ContentBuilder::new();
        let form = content.subject_with_identity(
            "form/frames",
            conduit_presentation::PresentationRole::Form,
            "Frames",
            "Frame Resource proof",
        );
        crate::portable_graph_projection::append_exact_graph(
            &form,
            &graph,
            Some(&document),
            None,
            &mut content,
        );
        let body = conduit_body::Body::born(
            proof.plan.source_document_id.clone(),
            proof.plan.checked_form_id.clone(),
            1,
            "sign/frame-born".into(),
        )
        .unwrap();
        let presentation = conduit_presentation::Presentation::new(
            1,
            conduit_presentation::PresentationBasis {
                body_id: Some(body.body_id),
                wake_id: None,
                source_document_id: Some(proof.plan.source_document_id.clone()),
                checked_form_id: Some(proof.plan.checked_form_id.clone()),
                expanded_form_id: Some(proof.plan.expanded_form_id.clone()),
                plan_id: Some(proof.plan.plan_id.clone()),
                active_play_id: None,
                sign_ids: vec![],
            },
            content.subjects,
            content.relationships,
            content.properties,
            content.text,
        )
        .unwrap();
        let projected = format!("{:?}", presentation.properties);
        for expected in [
            "RESOURCE image/rgba@1",
            "generation",
            "ReadPublished",
            "WriteCandidatePublish",
            "Play",
            "SingleWriterPublished",
            "arena/shared-read@1",
            "reader leases",
            "publication slots",
            "resource-",
            "Line",
        ] {
            assert!(
                projected.contains(expected),
                "missing {expected}: {projected}"
            );
        }
        assert_eq!(serde_json::to_vec(&proof.plan).unwrap(), before);
    }
}
