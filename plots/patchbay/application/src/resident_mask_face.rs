//! Exact inspected Mask-stage facts, independent of any action return route.
use super::PatchbayMaskTopology;
use alloc::{format, string::String};
use conduit_face::{
    FaceDisclosure, FaceDisclosureLevel, FaceFragment, FaceProperty, FacePropertyValue,
    FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
};

pub(super) fn append(fragment: &mut FaceFragment, root: &str, topology: &PatchbayMaskTopology) {
    for (name, value) in [
        ("mask-face-id", topology.face_id.as_str()),
        ("mask-body-plan-id", topology.body_plan_id.as_str()),
        ("mask-play-id", topology.active_play_id.as_str()),
    ] {
        fragment.properties.push(property(
            root,
            name,
            FacePropertyValue::Identity(value.into()),
        ));
    }
    for (index, stage) in topology.stages.iter().enumerate() {
        let subject = format!("{root}/mask-stage/{index}");
        fragment.subjects.push(FaceSubject {
            identity: subject.clone(),
            role: FaceRole::Info,
            name: if stage.available {
                "Mask stage available"
            } else {
                "Mask stage unavailable"
            }
            .into(),
        });
        fragment.relationships.push(FaceRelationship {
            source: root.into(),
            target: subject.clone(),
            kind: FaceRelationshipKind::Contains,
        });
        fragment.disclosures.push(FaceDisclosure {
            subject: subject.clone(),
            level: FaceDisclosureLevel::Context,
        });
        for (name, value) in [
            ("manifestation-id", &stage.manifestation_id),
            ("implementation-id", &stage.implementation_id),
            ("host-id", &stage.host_id),
            ("boot-id", &stage.boot_id),
            ("resource-pool-id", &stage.resource_pool_id),
            ("resource-class-id", &stage.resource_class_id),
        ] {
            fragment.properties.push(property(
                &subject,
                name,
                FacePropertyValue::Identity(value.clone()),
            ));
        }
        for (name, value) in [
            ("reserved-units", u64::from(stage.reserved_units)),
            (
                "maximum-active-instances",
                u64::from(stage.maximum_active_instances),
            ),
            ("maximum-queue-items", u64::from(stage.maximum_queue_items)),
            ("maximum-queue-bytes", u64::from(stage.maximum_queue_bytes)),
        ] {
            fragment
                .properties
                .push(property(&subject, name, FacePropertyValue::Count(value)));
        }
        fragment.properties.push(property(
            &subject,
            "available",
            FacePropertyValue::Flag(stage.available),
        ));
    }
}
fn property(subject: &str, name: &str, value: FacePropertyValue) -> FaceProperty {
    FaceProperty {
        subject: String::from(subject),
        name: String::from(name),
        value,
    }
}
