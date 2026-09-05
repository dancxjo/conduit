use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};

pub fn replay_value(seed: u8, profile: &str) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([seed; 32]),
        content_profile: kind_id(profile),
        access_class: ResourceClassId::from("conduit.resource/history-value@1"),
        extent: ResourceExtent {
            bytes: 4,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([seed.wrapping_add(1); 32]),
            expires_at: None,
        },
    }
}
