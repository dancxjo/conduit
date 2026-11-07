//! Descriptive member presentation claim, never possession or route authority.
//! Every owner use must recheck current admitted Part, latest credential,
//! observed offer and actual admitted Lines. Construction authenticates nothing.
use conduit_body::{
    MembershipCredential, PortableAdmissionReceipt, SPAWN_ADMISSION_RECEIPT_SCHEMA,
};
use conduit_core::{HostAdvertisement, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemberPresentationBasis {
    pub(crate) credential: MembershipCredential,
    pub(crate) host_advertisement: HostAdvertisement,
}
impl MemberPresentationBasis {
    pub(crate) fn from_admission(receipt: &PortableAdmissionReceipt) -> Result<Self, String> {
        if receipt.schema != SPAWN_ADMISSION_RECEIPT_SCHEMA
            || !receipt.membership_admitted
            || receipt.plan_created
            || receipt.play_created
        {
            return Err("presentation admission receipt has invalid membership semantics".into());
        }
        Self::new(
            receipt.credential.clone(),
            receipt.host_advertisement.clone(),
        )
    }

    /// Validate only bounded identity and internal coherence. The caller must
    /// independently verify a returning member before using this claim.
    pub(crate) fn new(
        credential: MembershipCredential,
        host_advertisement: HostAdvertisement,
    ) -> Result<Self, String> {
        let claim = Self {
            credential,
            host_advertisement,
        };
        claim.validate()?;
        Ok(claim)
    }

    /// Recheck deserialized claims without cloning or allocating on success.
    /// Validation is internal coherence only, never authenticated membership.
    pub(crate) fn validate(&self) -> Result<(), String> {
        let credential = &self.credential;
        let host_advertisement = &self.host_advertisement;
        let ids = [
            credential.credential_id.as_str(),
            credential.body_id.as_str(),
            credential.part_id.as_str(),
            credential.host_id.as_str(),
            credential.boot_id.as_str(),
            host_advertisement.profile.as_str(),
        ];
        if ids.iter().any(|id| id.is_empty() || id.len() > 256)
            || credential.host_id != host_advertisement.host_id
            || credential.boot_id != host_advertisement.boot_id
            || host_advertisement.protocol_version != PROTOCOL_VERSION
            || host_advertisement.offer_generation.0 == 0
        {
            return Err("presentation claim has invalid member Host scope".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt() -> PortableAdmissionReceipt {
        fn id<T: serde::de::DeserializeOwned>(text: &str) -> T {
            serde_json::from_value(serde_json::json!(text)).unwrap()
        }
        let host_advertisement =
            conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
                host_id: "host/presentation".into(),
                boot_id: "boot/presentation".into(),
                offer_generation: conduit_core::OfferGeneration(1),
            })
            .advertisement()
            .clone();
        PortableAdmissionReceipt {
            schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
            credential: MembershipCredential {
                credential_id: id("credential/presentation"),
                body_id: id("body/presentation"),
                part_id: id("part/presentation"),
                host_id: host_advertisement.host_id.clone(),
                boot_id: host_advertisement.boot_id.clone(),
                issued_at_millis: 1,
            },
            host_advertisement,
            membership_admitted: true,
            current_offers_available: false,
            plan_created: false,
            play_created: false,
        }
    }
    #[test]
    fn original_admission_converts_without_adding_authority_and_malformed_receipts_refuse() {
        let original = receipt();
        let basis = MemberPresentationBasis::from_admission(&original).unwrap();
        assert!(basis.credential == original.credential);
        assert!(basis.host_advertisement == original.host_advertisement);
        let mut wire = serde_json::to_value(&basis).unwrap();
        wire["credential"]["boot_id"] = serde_json::json!("boot/forged");
        let decoded: MemberPresentationBasis = serde_json::from_value(wire).unwrap();
        assert!(decoded.validate().is_err());
        for variation in 0..7 {
            let mut bad = receipt();
            match variation {
                0 => bad.schema = "other/receipt".into(),
                1 => bad.membership_admitted = false,
                2 => bad.plan_created = true,
                3 => bad.play_created = true,
                4 => bad.credential.boot_id = "boot/other".into(),
                5 => bad.credential.host_id = "host/other".into(),
                _ => bad.host_advertisement.offer_generation.0 = 0,
            }
            assert!(MemberPresentationBasis::from_admission(&bad).is_err());
        }
    }
}
