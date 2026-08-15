//! Canonical bounded WORLD-layer projection for the Patchbay front door.

use conduit_body::Body;
use conduit_presentation::{
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole,
};

use crate::{
    portable_projection::{append_sign, ContentBuilder},
    PartPresentationState, PartsView, PatchbayPresentation,
};

pub(super) fn append_body_parts(body: &Body, parts: &PartsView, content: &mut ContentBuilder) {
    let body_subject = content.subject_with_identity(
        format!("body/{}", body.body_id.as_str()),
        PresentationRole::Body,
        "Current Body",
        format!("Current Body {}", body.body_id.as_str()),
    );
    identity(content, &body_subject, "body-id", body.body_id.as_str());
    content.property(
        &body_subject,
        "awake",
        PresentationPropertyValue::Flag(parts.awake),
    );
    for row in &parts.parts {
        let part_subject = content.subject_with_identity(
            format!("part/{}", row.details.part_id.as_str()),
            PresentationRole::Part,
            &row.label,
            format!("{} {:?}", row.label, row.state),
        );
        content.contains(&body_subject, &part_subject);
        identity(
            content,
            &part_subject,
            "part-id",
            row.details.part_id.as_str(),
        );
        text(
            content,
            &part_subject,
            "membership-state",
            match row.state {
                PartPresentationState::Here => "here",
                PartPresentationState::Attached => "attached",
                PartPresentationState::Offline => "offline",
            },
        );
        for (name, value) in [
            ("current", row.available),
            ("in-plan", row.in_plan),
            ("playing", row.playing),
        ] {
            content.property(&part_subject, name, PresentationPropertyValue::Flag(value));
        }
        if let Some(proof) = &row.details.proof_reference {
            identity(content, &part_subject, "membership-proof", proof);
        }
        for sign in &row.details.evidence_signs {
            append_sign(&part_subject, sign, content);
        }
        if let Some(generation) = row.details.offer_generation {
            content.property(
                &part_subject,
                "offer-generation",
                PresentationPropertyValue::Count(generation.0),
            );
        }
        if let Some(sequence) = row.details.presence_sequence {
            content.property(
                &part_subject,
                "freshness-sequence",
                PresentationPropertyValue::Count(sequence),
            );
        }
        if let Some(binding) = &row.details.presence_session_binding {
            identity(content, &part_subject, "binding-id", binding);
        }
        if let (Some(observed_at), Some(expires_at)) = (
            row.details.presence_observed_at_millis,
            row.details.presence_expires_at_millis,
        ) {
            text(
                content,
                &part_subject,
                "freshness",
                &format!("observed-at-millis={observed_at} expires-at-millis={expires_at}"),
            );
        }
        if let (Some(host_id), Some(boot_id)) = (&row.details.host_id, &row.details.boot_id) {
            let host_subject = host_identity(host_id.as_str(), boot_id.as_str());
            content.subject_with_identity(
                &host_subject,
                PresentationRole::Host,
                host_id.as_str(),
                format!("Host {} boot {}", host_id.as_str(), boot_id.as_str()),
            );
            content.relationships.push(PresentationRelationship {
                source: part_subject,
                target: host_subject.clone(),
                kind: PresentationRelationshipKind::Realizes,
            });
        }
    }
    for row in &parts.wants_to_join {
        let candidate = content.subject_with_identity(
            format!("candidate/{}", row.candidate_id.as_str()),
            PresentationRole::Candidate,
            &row.label,
            format!("Admission candidate {}", row.label),
        );
        content.relationships.push(PresentationRelationship {
            source: candidate.clone(),
            target: body_subject.clone(),
            kind: PresentationRelationshipKind::Observes,
        });
        identity(
            content,
            &candidate,
            "candidate-id",
            row.candidate_id.as_str(),
        );
        identity(content, &candidate, "host-id", row.host_id.as_str());
        identity(content, &candidate, "boot-id", row.boot_id.as_str());
        content.property(
            &candidate,
            "offer-generation",
            PresentationPropertyValue::Count(row.offer_generation.0),
        );
        content.property(
            &candidate,
            "capability-count",
            PresentationPropertyValue::Count(row.capabilities as u64),
        );
        text(content, &candidate, "membership-state", "wants-to-join");
        for sign in &row.evidence_signs {
            append_sign(&candidate, sign, content);
        }
        for capability in &row.capability_offers {
            append_capability(
                &candidate,
                row.host_id.as_str(),
                row.boot_id.as_str(),
                capability,
                content,
            );
        }
    }
}

pub(super) fn append_observatory(
    presentation: &PatchbayPresentation,
    document: &str,
    content: &mut ContentBuilder,
) {
    let Some(topology) = &presentation.topology else {
        return;
    };
    for host in &topology.hosts {
        let subject_identity = host_identity(host.host_id.as_str(), host.boot_id.as_str());
        let subject = content.subject_with_identity(
            subject_identity,
            PresentationRole::Host,
            host.host_id.as_str(),
            format!(
                "Host {} boot {}",
                host.host_id.as_str(),
                host.boot_id.as_str()
            ),
        );
        content.describes(&subject, document);
        identity(content, &subject, "host-id", host.host_id.as_str());
        identity(content, &subject, "boot-id", host.boot_id.as_str());
        identity(content, &subject, "profile-id", host.profile.as_str());
        content.property(
            &subject,
            "offer-generation",
            PresentationPropertyValue::Count(host.offer_generation.0),
        );
        for (name, count) in [
            ("capability-count", host.capability_count),
            ("resource-count", host.resources.len()),
            ("planner-capability-count", host.planner_capabilities.len()),
        ] {
            content.property(
                &subject,
                name,
                PresentationPropertyValue::Count(count as u64),
            );
        }
        for (index, resource) in host.resources.iter().enumerate() {
            identity(
                content,
                &subject,
                &format!("resource-{index}-pool-id"),
                resource.pool_id.as_str(),
            );
            identity(
                content,
                &subject,
                &format!("resource-{index}-class-id"),
                resource.class_id.as_str(),
            );
            content.property(
                &subject,
                &format!("resource-{index}-capacity-units"),
                PresentationPropertyValue::Count(u64::from(resource.capacity_units)),
            );
        }
        text(
            content,
            &subject,
            "operational-state",
            &format!("{:?}", host.state),
        );
    }
    for capability in &topology.capabilities {
        let subject = content.subject_with_identity(
            format!(
                "capability/{}/{}/{}",
                capability.host_id.as_str(),
                capability.boot_id.as_str(),
                capability.capability_id.as_str()
            ),
            PresentationRole::Capability,
            capability.kind_id.as_str(),
            format!("Capability {}", capability.capability_id.as_str()),
        );
        content.contains(
            &host_identity(capability.host_id.as_str(), capability.boot_id.as_str()),
            &subject,
        );
        identity(
            content,
            &subject,
            "capability-id",
            capability.capability_id.as_str(),
        );
        identity(content, &subject, "kind-id", capability.kind_id.as_str());
        text(
            content,
            &subject,
            "availability",
            &format!("{:?}", capability.availability),
        );
        text(
            content,
            &subject,
            "freshness",
            &format!("{:?}", capability.freshness),
        );
    }
    for line in &topology.lines {
        let subject = content.subject_with_identity(
            format!("line/{}", line.offer.line_id.as_str()),
            PresentationRole::Line,
            line.offer.line_id.as_str(),
            format!("Line {}", line.offer.line_id.as_str()),
        );
        content.describes(&subject, document);
        identity(content, &subject, "line-id", line.offer.line_id.as_str());
        identity(
            content,
            &subject,
            "binding-id",
            line.offer.binding.binding_id.as_str(),
        );
        for (name, value) in [
            ("source-host-id", line.offer.binding.source.host_id.as_str()),
            ("source-boot-id", line.offer.binding.source.boot_id.as_str()),
            ("sink-host-id", line.offer.binding.sink.host_id.as_str()),
            ("sink-boot-id", line.offer.binding.sink.boot_id.as_str()),
            (
                "base-instance-id",
                line.offer.binding.base_instance_id.as_str(),
            ),
        ] {
            identity(content, &subject, name, value);
        }
        for endpoint in [&line.offer.binding.source, &line.offer.binding.sink] {
            content.relationships.push(PresentationRelationship {
                source: subject.clone(),
                target: host_identity(endpoint.host_id.as_str(), endpoint.boot_id.as_str()),
                kind: PresentationRelationshipKind::Connects,
            });
        }
        content.property(
            &subject,
            "base",
            PresentationPropertyValue::ConnectionBase(line.offer.binding.base),
        );
        for (name, value) in [
            (
                "maximum-in-flight-items",
                u64::from(line.offer.binding.limits.maximum_in_flight_items),
            ),
            (
                "maximum-payload-bytes",
                u64::from(line.offer.binding.limits.maximum_payload_bytes),
            ),
            (
                "maximum-buffered-bytes",
                u64::from(line.offer.binding.limits.maximum_buffered_bytes),
            ),
            (
                "maximum-frame-bytes",
                u64::from(line.offer.binding.limits.maximum_frame_bytes),
            ),
        ] {
            content.property(&subject, name, PresentationPropertyValue::Count(value));
        }
        text(
            content,
            &subject,
            "operational-state",
            &format!("{:?}", line.state),
        );
        text(
            content,
            &subject,
            "availability",
            &format!("{:?}", line.offer.availability.availability),
        );
        append_sign(&subject, &line.offer.availability.sign_id, content);
    }
    for sign in &topology.signs {
        append_sign(document, &sign.sign_id, content);
    }
}

fn host_identity(host: &str, boot: &str) -> String {
    format!("host/{host}/boot/{boot}")
}

fn append_capability(
    owner: &str,
    host: &str,
    boot: &str,
    capability: &crate::PartCapability,
    content: &mut ContentBuilder,
) {
    let subject = content.subject_with_identity(
        format!(
            "capability/{host}/{boot}/{}",
            capability.capability_id.as_str()
        ),
        PresentationRole::Capability,
        capability.kind_id.as_str(),
        format!("Capability {}", capability.capability_id.as_str()),
    );
    content.contains(owner, &subject);
    identity(
        content,
        &subject,
        "capability-id",
        capability.capability_id.as_str(),
    );
    identity(content, &subject, "kind-id", capability.kind_id.as_str());
    text(content, &subject, "availability", "advertised");
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
