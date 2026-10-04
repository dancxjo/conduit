//! Exact directional Line offers for the authenticated native owner carrier.
//! The caller holds the accepted Face socket and a bounded return grant on
//! the same pinned listener when these offers are made.

use conduit_body::PortableAdmissionReceipt;
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
// The ConduitOS bounded WebSocket carries one 8 KiB wire frame with a
// fourteen-byte envelope. Larger return documents use ordered chunks.
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

pub(super) fn offer_pair(
    owner: &HostAdvertisement,
    guest: &PortableAdmissionReceipt,
    binding_reference: &str,
) -> (LineOffer, LineOffer) {
    let credential = &guest.credential;
    let face = line(
        "face",
        binding_reference,
        credential.credential_id.as_str(),
        owner,
        &guest.host_advertisement,
    );
    let returned = line(
        "return",
        binding_reference,
        credential.credential_id.as_str(),
        &guest.host_advertisement,
        owner,
    );
    (face, returned)
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
                maximum_payload_bytes: if direction == "face" {
                    MAX_NATIVE_FRAME_BYTES as u32
                } else {
                    64 * 1024
                },
                maximum_frame_bytes: MAX_NATIVE_FRAME_BYTES as u32,
                maximum_buffered_bytes: 128 * 1024,
            },
        },
        contract: CONTRACT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{MembershipCredential, SPAWN_ADMISSION_RECEIPT_SCHEMA};
    use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration};

    fn host(id: &str) -> HostAdvertisement {
        HostAdvertisement {
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
        let receipt = PortableAdmissionReceipt {
            schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
            credential: MembershipCredential {
                credential_id: serde_json::from_str("\"credential/native\"").unwrap(),
                body_id: serde_json::from_str("\"body/one\"").unwrap(),
                part_id: serde_json::from_str("\"part/native\"").unwrap(),
                host_id: guest.host_id.clone(),
                boot_id: guest.boot_id.clone(),
                issued_at_millis: 1,
            },
            host_advertisement: guest.clone(),
            membership_admitted: true,
            current_offers_available: false,
            plan_created: false,
            play_created: false,
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
        assert_eq!(face.binding.limits.maximum_payload_bytes, 8192 - 14);
        assert_eq!(returned.binding.limits.maximum_frame_bytes, 8192 - 14);
        assert_eq!(returned.binding.limits.maximum_payload_bytes, 64 * 1024);
    }
}
