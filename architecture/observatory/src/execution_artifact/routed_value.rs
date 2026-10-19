//! Complete value bytes correlated with a retained routing Sign. Correlation
//! does not authenticate execution or establish domain-specific value validity.
use super::*;
use alloc::vec::Vec;
use conduit_core::KindId;
pub const ROUTED_VALUE_ARTIFACT_SCHEMA: &str = "conduit.routed-value/artifact@1";
pub const MAXIMUM_ROUTED_VALUE_BYTES: usize = 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutedValueArtifact {
    pub schema: String,
    pub sign: SignArtifact,
    pub value_kind: KindId,
    pub bytes: Vec<u8>,
}
impl RoutedValueArtifact {
    pub fn new(sign: SignArtifact, bytes: Vec<u8>) -> Result<Self, String> {
        let value_kind = declared_kind(&sign)?.clone();
        let artifact = Self {
            schema: ROUTED_VALUE_ARTIFACT_SCHEMA.into(),
            sign,
            value_kind,
            bytes,
        };
        validate_routed_value_artifact(&artifact)?;
        Ok(artifact)
    }
}
fn declared_kind(artifact: &SignArtifact) -> Result<&KindId, String> {
    super::routing::declared_kind(artifact)
}
pub fn validate_routed_value_artifact(artifact: &RoutedValueArtifact) -> Result<(), String> {
    if artifact.schema != ROUTED_VALUE_ARTIFACT_SCHEMA {
        return Err("unsupported routed value artifact schema".into());
    }
    if artifact.bytes.len() > MAXIMUM_ROUTED_VALUE_BYTES {
        return Err("routed value artifact extent exceeded".into());
    }
    validate_sign_artifact(&artifact.sign)?;
    if declared_kind(&artifact.sign)? != &artifact.value_kind {
        return Err("routed value kind differs from its producing Port".into());
    }
    let evidence =
        super::routing::evidence(&artifact.sign.sign.kind).expect("checked routing kind");
    if evidence.digest_domain != "conduit/routed-value-content@1"
        || evidence.digest_scheme != "core/semantic-sha256@1"
    {
        return Err("unsupported routed value digest policy".into());
    }
    if artifact.bytes.len() != evidence.byte_length as usize
        || conduit_core::semantic_digest(&evidence.digest_domain, &artifact.bytes)
            != evidence.digest
    {
        return Err("routed value bytes differ from their execution content Sign".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn artifact() -> RoutedValueArtifact {
        let (_, _, mut sign) = super::super::tests::exact_artifacts();
        let connection = sign.play.as_ref().unwrap().plan.fragments[0].connections[0].clone();
        let bytes = alloc::vec![42];
        sign.sign.placement_id = Some(connection.source_placement_id);
        sign.sign.connection_id = Some(connection.connection_id);
        sign.sign.kind = ObservationKind::RoutedContent {
            evidence: conduit_core::RoutedContentEvidence {
                native_event_sequence: 42,
                output_port: connection.source_port_id,
                byte_length: bytes.len() as u32,
                digest_domain: "conduit/routed-value-content@1".into(),
                digest_scheme: "core/semantic-sha256@1".into(),
                digest: conduit_core::semantic_digest("conduit/routed-value-content@1", &bytes),
            },
        };
        RoutedValueArtifact::new(sign, bytes).unwrap()
    }
    #[test]
    fn complete_bytes_roundtrip_and_context_kind_digest_mutations_refuse() {
        let good = artifact();
        let bytes = serde_json::to_vec(&good).unwrap();
        let decoded: RoutedValueArtifact = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(good, decoded);
        validate_routed_value_artifact(&decoded).unwrap();
        for mutation in 0..6 {
            let mut bad = good.clone();
            match mutation {
                0 => bad.bytes[0] ^= 1,
                1 => bad.bytes.push(0),
                2 => bad.value_kind = "value/forged".into(),
                3 => bad.sign.sign.active_play_id = None,
                4 => bad.schema.push('x'),
                _ => {
                    if let ObservationKind::RoutedContent { evidence } = &mut bad.sign.sign.kind {
                        evidence.digest_scheme = "unknown".into();
                    }
                }
            }
            assert!(validate_routed_value_artifact(&bad).is_err());
        }
    }
}

#[cfg(test)]
#[path = "routed_value/tracks.rs"]
mod track_tests;
