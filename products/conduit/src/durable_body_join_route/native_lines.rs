//! Exact directional Line offers for the authenticated native owner carrier.
//! The caller holds the accepted Face socket and a bounded return grant on
//! the same pinned listener when these offers are made.

use crate::durable_host::owner::MemberPresentationBasis;
use conduit_core::{
    AuthorityGrantId, BaseImplementationId, BaseInstanceId, CredentialReferenceId,
    HostAdvertisement, LineAvailability, LineAvailabilitySign, LineContinuation, LineContract,
    LineDuplex, LineId, LineOffer, LineOrdering, LineReliability, LineScope, LineSecurity,
    LineTrafficShape, LinkAuthorityReference, LinkBinding, LinkBindingId, LinkCredentialReference,
    LinkEndpoint, LinkEndpointId, LinkLimits, SignId,
};
use conduit_std_host::secure_websocket::SecureWebSocketListener;

const CONTRACT: LineContract = LineContract {
    scope: LineScope::RoutedNetwork,
    traffic_shape: LineTrafficShape::Message,
    duplex: LineDuplex::FullDuplex,
    ordering: LineOrdering::Ordered,
    reliability: LineReliability::Reliable,
    continuation: LineContinuation::None,
    security: LineSecurity::AuthenticatedEncrypted,
};
// Physical WebSocket chunk capacity, not the logical bounded-document Line
// frame. The adapter reassembles larger logical messages from ordered chunks.
pub(super) const MAX_NATIVE_FRAME_BYTES: usize = 8192 - 14;

pub(super) fn binding_reference(
    listener: &SecureWebSocketListener,
    grant_reference: &str,
) -> String {
    let certificate = listener.certificate_binding_sha256();
    let certificate: String = certificate[..12]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{certificate}/{grant_reference}")
}

pub(crate) fn offer_pair(
    owner: &HostAdvertisement,
    guest: &MemberPresentationBasis,
    binding_reference: &str,
) -> (LineOffer, LineOffer) {
    let credential = &guest.credential;
    let mut face = line(
        "face",
        binding_reference,
        credential.credential_id.as_str(),
        owner,
        &guest.host_advertisement,
    );
    let profile = conduit_conduitos_mask_offer::NativeMaskStorageProfile::owner_document_profile(
        &guest.host_advertisement,
    );
    if profile == conduit_conduitos_mask_offer::NativeMaskStorageProfile::Collection {
        face.binding.limits.maximum_payload_bytes = profile.owner_document_bytes() as u32;
        face.binding.limits.maximum_frame_bytes = profile.owner_document_bytes() as u32;
        face.binding.limits.maximum_buffered_bytes = (2 * profile.owner_document_bytes()) as u32;
    }
    let returned = line(
        "return",
        binding_reference,
        credential.credential_id.as_str(),
        &guest.host_advertisement,
        owner,
    );
    (face, returned)
}

/// Logical document limits remain distinct from smaller physical wire chunks.
/// The selected profile's receiver must support its complete admitted document.
pub(super) fn offer_terminal(
    owner: &HostAdvertisement,
    peer: &MemberPresentationBasis,
    binding_reference: &str,
) -> (LineOffer, LineOffer, LineOffer) {
    let (mut face, mut show) = offer_pair(owner, peer, binding_reference);
    let mut interaction = line(
        "interaction",
        binding_reference,
        peer.credential.credential_id.as_str(),
        &peer.host_advertisement,
        owner,
    );
    for line in [&mut face, &mut show, &mut interaction] {
        line.binding.limits.maximum_frame_bytes = line.binding.limits.maximum_payload_bytes;
    }
    (face, show, interaction)
}

fn line(
    direction: &str,
    binding_reference: &str,
    credential_id: &str,
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
) -> LineOffer {
    let line_id = LineId::from(format!("line/native-owner/{binding_reference}/{direction}"));
    let binding_id = LinkBindingId::from(format!(
        "binding/native-owner/{binding_reference}/{direction}"
    ));
    LineOffer {
        availability: LineAvailabilitySign {
            line_id: line_id.clone(),
            binding_id: binding_id.clone(),
            availability: LineAvailability::Ready,
            sign_id: SignId::from(format!(
                "sign/native-owner/{binding_reference}/{direction}/ready"
            )),
        },
        line_id,
        binding: LinkBinding {
            binding_id,
            source: LinkEndpoint {
                host_id: source.host_id.clone(),
                boot_id: source.boot_id.clone(),
                endpoint_id: LinkEndpointId::from(format!(
                    "endpoint/native-owner/{binding_reference}/{direction}/source"
                )),
            },
            sink: LinkEndpoint {
                host_id: sink.host_id.clone(),
                boot_id: sink.boot_id.clone(),
                endpoint_id: LinkEndpointId::from(format!(
                    "endpoint/native-owner/{binding_reference}/{direction}/sink"
                )),
            },
            base: BaseImplementationId::from("conduit.base/authenticated-tls-websocket@1"),
            base_instance_id: BaseInstanceId::from(format!(
                "base-instance/native-owner/{binding_reference}"
            )),
            credential: LinkCredentialReference::Opaque(CredentialReferenceId::from(credential_id)),
            authority: LinkAuthorityReference::Grant(AuthorityGrantId::from(format!(
                "grant/native-owner/{binding_reference}"
            ))),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64 * 1024,
                // One complete logical adapter message; send/receive retain
                // their separate MAX_NATIVE_FRAME_BYTES physical chunk bound.
                maximum_frame_bytes: 64 * 1024,
                maximum_buffered_bytes: 128 * 1024,
            },
        },
        contract: CONTRACT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::MembershipCredential;
    use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration};

    fn host(id: &str) -> HostAdvertisement {
        HostAdvertisement {
            declared_name: None,
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id: HostId::from(id),
            boot_id: BootId::from(format!("boot/{id}")),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("test"),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        }
    }

    #[test]
    fn one_pinned_carrier_has_distinct_directional_bindings() {
        let owner = host("host/owner");
        let guest = host("host/native");
        let receipt = MemberPresentationBasis {
            credential: MembershipCredential {
                credential_id: serde_json::from_str("\"credential/native\"").unwrap(),
                body_id: serde_json::from_str("\"body/one\"").unwrap(),
                part_id: serde_json::from_str("\"part/native\"").unwrap(),
                host_id: guest.host_id.clone(),
                boot_id: guest.boot_id.clone(),
                issued_at_millis: 1,
            },
            host_advertisement: guest.clone(),
        };
        let (face, returned) = offer_pair(&owner, &receipt, "pinned/grant/0");
        assert!(face.validate_sign_identity());
        assert!(returned.validate_sign_identity());
        assert_ne!(face.line_id, returned.line_id);
        assert_ne!(face.binding.binding_id, returned.binding.binding_id);
        assert_eq!(face.binding.source.host_id, owner.host_id);
        assert_eq!(face.binding.sink.host_id, guest.host_id);
        assert_eq!(returned.binding.source.host_id, guest.host_id);
        assert_eq!(returned.binding.sink.host_id, owner.host_id);
        assert_eq!(face.contract.security, LineSecurity::AuthenticatedEncrypted);
        assert_eq!(face.binding.limits.maximum_payload_bytes, 64 * 1024);
        assert_eq!(returned.binding.limits.maximum_frame_bytes, 64 * 1024);
        assert_eq!(returned.binding.limits.maximum_payload_bytes, 64 * 1024);
        let planned = conduit_conduitos_mask_offer::prepare_stage(
            conduit_conduitos_mask_offer::Adapter::Native,
            &guest.host_id,
            &guest.boot_id,
            guest.offer_generation.0,
            None,
        )
        .unwrap()
        .planned_mask;
        let choices = [
            (
                conduit_core::PortDirection::Input,
                &planned.mask.face_input.front_port_id,
                &face,
            ),
            (
                conduit_core::PortDirection::Output,
                &planned.mask.show_output.front_port_id,
                &returned,
            ),
        ]
        .map(
            |(direction, port, line)| conduit_planner::ExternalForeLineChoice {
                host_id: guest.host_id.clone(),
                boot_id: guest.boot_id.clone(),
                direction,
                front_port_id: port.clone(),
                track: conduit_core::ConnectionTrack::Payload,
                peer_host_id: owner.host_id.clone(),
                peer_boot_id: owner.boot_id.clone(),
                line_id: line.line_id.clone(),
            },
        );
        let bases = [face.binding.base.clone()];
        assert!(conduit_planner::bind_external_fore_lines(
            &planned.plan,
            &choices,
            &[face.clone(), returned.clone()],
            &bases
        )
        .is_ok());
        let mut malformed = face.clone();
        malformed.binding.limits.maximum_frame_bytes = MAX_NATIVE_FRAME_BYTES as u32;
        assert!(matches!(
            conduit_planner::bind_external_fore_lines(
                &planned.plan,
                &choices,
                &[malformed, returned.clone()],
                &bases
            ),
            Err(conduit_planner::PlannerError::InvalidLineOffer(_))
        ));
        let (terminal_face, terminal_show, interaction) =
            offer_terminal(&owner, &receipt, "pinned/grant/0");
        assert_ne!(interaction.line_id, terminal_face.line_id);
        assert_ne!(interaction.line_id, terminal_show.line_id);
        assert_eq!(interaction.binding.source.host_id, guest.host_id);
        assert_eq!(interaction.binding.sink.host_id, owner.host_id);
        for line in [terminal_face, terminal_show, interaction] {
            assert!(line.validate_sign_identity());
            assert_eq!(line.binding.limits.maximum_frame_bytes, 64 * 1024);
            assert_eq!(line.contract.security, LineSecurity::AuthenticatedEncrypted);
        }
        let mut collection = receipt.clone();
        collection.host_advertisement.capabilities =
            conduit_conduitos_mask_offer::native_read_only_implementation_closure(
                conduit_conduitos_mask_offer::NativeMaskStorageProfile::Collection,
            );
        assert_eq!(
            conduit_conduitos_mask_offer::NativeMaskStorageProfile::owner_document_profile(
                &collection.host_advertisement
            ),
            conduit_conduitos_mask_offer::NativeMaskStorageProfile::Collection
        );
        let (collection_face, collection_return) =
            offer_pair(&owner, &collection, "pinned/collection/0");
        assert_eq!(
            collection_face.binding.limits.maximum_payload_bytes,
            384 * 1024
        );
        for line in [collection_face, collection_return] {
            assert!(
                line.binding.limits.maximum_frame_bytes
                    >= line.binding.limits.maximum_payload_bytes
            );
        }
        // Profile capacity consistency only. The hosted presentation client is
        // currently Standard-only; larger Collection delivery is not asserted.
    }
}
