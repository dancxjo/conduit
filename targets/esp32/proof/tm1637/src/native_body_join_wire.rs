//! Strict observation-only subset of the canonical request carrier. No rich
//! capability/Kind/resource deserializer is reachable through this decoder.
use alloc::{string::String, vec::Vec};
use conduit_body::{BodyId, PortableSpawnAdmissionRequest, SpawnInvitationId};
use conduit_core::{BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration};
use serde::{
    de::{Error, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
pub(crate) const OBSERVER_PROFILE: &str = "conduit.environment/observer-admission@1";
const MAX_BYTES: usize = 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    invitation_id: SpawnInvitationId,
    body_id: BodyId,
    host_advertisement: Advertisement,
    nonce: Bytes<32>,
    signature: Bytes<64>,
    membership_admitted: bool,
    plan_created: bool,
    play_created: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Advertisement {
    protocol_version: u16,
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    profile: HostProfileId,
    #[serde(default)]
    bases: EmptyArray,
    resources: EmptyArray,
    capabilities: EmptyArray,
    #[serde(default)]
    planner_capabilities: EmptyArray,
}
struct Bytes<const N: usize>([u8; N]);
impl<'de, const N: usize> Deserialize<'de> for Bytes<N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BytesVisitor<const N: usize>;
        impl<'de, const N: usize> Visitor<'de> for BytesVisitor<N> {
            type Value = Bytes<N>;
            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(formatter, "exactly {N} bytes or their hexadecimal spelling")
            }
            fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
                if value.len() != N * 2 {
                    return Err(E::custom("observer byte length"));
                }
                let mut bytes = [0; N];
                for (out, pair) in bytes.iter_mut().zip(value.as_bytes().as_chunks::<2>().0) {
                    fn digit(byte: u8) -> Option<u8> {
                        match byte {
                            b'0'..=b'9' => Some(byte - b'0'),
                            b'a'..=b'f' => Some(byte - b'a' + 10),
                            b'A'..=b'F' => Some(byte - b'A' + 10),
                            _ => None,
                        }
                    }
                    *out = (digit(pair[0])
                        .ok_or_else(|| E::custom("observer hexadecimal byte"))?
                        << 4)
                        | digit(pair[1]).ok_or_else(|| E::custom("observer hexadecimal byte"))?;
                }
                Ok(Bytes(bytes))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut bytes = [0; N];
                for byte in &mut bytes {
                    *byte = access
                        .next_element()?
                        .ok_or_else(|| A::Error::custom("observer byte length"))?;
                }
                if access.next_element::<RejectElement>()?.is_some() {
                    return Err(A::Error::custom("observer byte length"));
                }
                Ok(Bytes(bytes))
            }
        }
        deserializer.deserialize_any(BytesVisitor::<N>)
    }
}
#[derive(Default)]
struct EmptyArray;
struct RejectElement;
impl<'de> Deserialize<'de> for RejectElement {
    fn deserialize<D: Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
        Err(D::Error::custom("observer offers must be empty"))
    }
}
impl<'de> Deserialize<'de> for EmptyArray {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EmptyVisitor;
        impl<'de> Visitor<'de> for EmptyVisitor {
            type Value = EmptyArray;
            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("an empty observer offer array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                if access.next_element::<RejectElement>()?.is_some() {
                    return Err(A::Error::custom("observer offers must be empty"));
                }
                Ok(EmptyArray)
            }
        }
        deserializer.deserialize_seq(EmptyVisitor)
    }
}
pub(crate) fn decode(bytes: &[u8]) -> Result<PortableSpawnAdmissionRequest, &'static str> {
    if bytes.len() > MAX_BYTES {
        return Err("body-join-request-capacity");
    }
    let request: Request =
        serde_json::from_slice(bytes).map_err(|_| "body-join-request-encoding")?;
    let Advertisement {
        protocol_version,
        host_id,
        boot_id,
        offer_generation,
        profile,
        bases: EmptyArray,
        resources: EmptyArray,
        capabilities: EmptyArray,
        planner_capabilities: EmptyArray,
    } = request.host_advertisement;
    if protocol_version != conduit_core::PROTOCOL_VERSION || offer_generation.0 == 0 {
        return Err("body-join-observer-protocol");
    }
    let request = PortableSpawnAdmissionRequest {
        schema: if request.schema == OBSERVER_PROFILE {
            String::from(conduit_body::SPAWN_ADMISSION_REQUEST_SCHEMA)
        } else {
            request.schema
        },
        invitation_id: request.invitation_id,
        body_id: request.body_id,
        host_advertisement: HostAdvertisement {
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id,
            boot_id,
            offer_generation,
            profile,
            bases: Vec::new(),
            resources: Vec::new(),
            capabilities: Vec::new(),
            planner_capabilities: Vec::new(),
        },
        nonce: request.nonce.0,
        signature: request.signature.0.to_vec(),
        membership_admitted: request.membership_admitted,
        plan_created: request.plan_created,
        play_created: request.play_created,
    };
    request
        .admission_proof()
        .map_err(|_| "body-join-request-proof")?;
    Ok(request)
}
