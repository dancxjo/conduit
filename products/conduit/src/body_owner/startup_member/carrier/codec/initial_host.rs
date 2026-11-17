//! Compare the actual Host envelope with the existing borrowed Core contract.
use conduit_core::{HostAdvertisement, HostAdvertisementEqualitySeed};
use serde::de::{DeserializeSeed, Error, MapAccess, Visitor};

fn compare(bytes: &[u8], expected: &HostAdvertisement) -> Result<(), String> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    Envelope(expected)
        .deserialize(&mut decoder)
        .map_err(|_| "startup authenticated Host context differs from selection")?;
    decoder
        .end()
        .map_err(|_| "startup Host trailing input".into())
}
struct Envelope<'a>(&'a HostAdvertisement);
impl<'de> DeserializeSeed<'de> for Envelope<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for Envelope<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("actual Host envelope")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut seen = 0u8;
        while let Some(field) = map.next_key_seed(Field)? {
            let bit = 1 << field;
            if seen & bit != 0 {
                return Err(A::Error::custom("duplicate Host field"));
            }
            seen |= bit;
            match field {
                0 => {
                    if map.next_value::<&str>()? != "host" {
                        return Err(A::Error::custom("Host kind"));
                    }
                }
                1 => {
                    if map.next_value::<u16>()? != 1 {
                        return Err(A::Error::custom("Host protocol"));
                    }
                }
                2..=4 => {
                    let _: serde::de::IgnoredAny = map.next_value()?;
                }
                5 => map.next_value_seed(
                    HostAdvertisementEqualitySeed::new(self.0)
                        .ok_or_else(|| A::Error::custom("unsupported Host comparison"))?,
                )?,
                6 => {
                    if map.next_value::<[&str; 1]>()? != ["conduit-line/authenticated-tls-stream@1"]
                    {
                        return Err(A::Error::custom("Host secure line"));
                    }
                }
                _ => unreachable!(),
            }
        }
        if seen != 127 {
            return Err(A::Error::custom("missing Host field"));
        }
        Ok(())
    }
}
struct Field;
impl<'de> DeserializeSeed<'de> for Field {
    type Value = u8;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<u8, D::Error> {
        d.deserialize_identifier(self)
    }
}
impl Visitor<'_> for Field {
    type Value = u8;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Host field")
    }
    fn visit_str<E: Error>(self, value: &str) -> Result<u8, E> {
        match value {
            "kind" => Ok(0),
            "protocol" => Ok(1),
            "friendly_label" => Ok(2),
            "target_id" => Ok(3),
            "image_content_digest" => Ok(4),
            "advertisement" => Ok(5),
            "lines" => Ok(6),
            _ => Err(E::custom("unknown Host field")),
        }
    }
}

/// The actual rendezvous producer uses the canonical serde Host writer. Rich
/// observations may borrow that exact encoding; noncanonical encodings refuse.
pub(super) fn observe(
    bytes: &[u8],
    expected: &HostAdvertisement,
) -> Result<Option<([u8; 32], usize)>, String> {
    if conduit_core::bounded_host_advertisement_comparison_supported(expected) {
        compare(bytes, expected)?;
        return Ok(None);
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawHost<'a> {
        kind: &'a str,
        protocol: u16,
        friendly_label: &'a str,
        target_id: &'a str,
        image_content_digest: &'a str,
        #[serde(borrow)]
        advertisement: &'a serde_json::value::RawValue,
        lines: [&'a str; 1],
    }
    let host: RawHost<'_> =
        serde_json::from_slice(bytes).map_err(|_| "invalid bounded rich Host envelope")?;
    if host.kind != "host"
        || host.protocol != 1
        || host.lines != ["conduit-line/authenticated-tls-stream@1"]
    {
        return Err("invalid startup rich Host provider".into());
    }
    let _ = (
        host.friendly_label,
        host.target_id,
        host.image_content_digest,
    );
    use sha2::Digest;
    let raw = host.advertisement.get().as_bytes();
    Ok(Some((sha2::Sha256::digest(raw).into(), raw.len())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;
    fn wire(ad: &HostAdvertisement) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"kind":"host","protocol":1,"friendly_label":"Member","target_id":"fixture","image_content_digest":"fixture","advertisement":ad,"lines":["conduit-line/authenticated-tls-stream@1"]})).unwrap()
    }
    #[test]
    fn actual_checked_timer_and_speaker_offer_use_borrowed_canonical_observation() {
        for speaker in [false, true] {
            let host = crate::durable_host_control::startup_member::test_fixture::selected_host(
                "fixture/member",
                "boot/member",
                speaker,
            );
            let ad = host.advertisement();
            assert!(!conduit_core::bounded_host_advertisement_comparison_supported(ad));
            // Use the actual producer's serde struct shape, not Value (which sorts keys).
            #[derive(serde::Serialize)]
            struct Host<'a> {
                kind: &'static str,
                protocol: u16,
                friendly_label: &'static str,
                target_id: &'static str,
                image_content_digest: &'static str,
                advertisement: &'a HostAdvertisement,
                lines: [&'static str; 1],
            }
            let bytes = serde_json::to_vec(&Host {
                kind: "host",
                protocol: 1,
                friendly_label: "Member",
                target_id: "fixture",
                image_content_digest: "fixture",
                advertisement: ad,
                lines: ["conduit-line/authenticated-tls-stream@1"],
            })
            .unwrap();
            let expected = serde_json::to_vec(ad).unwrap();
            let observed = observe(&bytes, ad).unwrap().unwrap();
            assert_eq!(
                observed,
                (sha2::Sha256::digest(&expected).into(), expected.len())
            );
            let mut foreign = ad.clone();
            foreign.boot_id = "boot/foreign".into();
            let changed = observe(&wire(&foreign), ad).unwrap().unwrap();
            assert_ne!(changed.0, observed.0);
            assert!(observe(&bytes[..bytes.len() - 1], ad).is_err());
        }
    }
}
