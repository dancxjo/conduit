//! Cold startup recovery over a genuine originally selected Root/NOR prefix.
use super::*;

pub(super) fn assert_cold_recovery(
    selected: [u8; a::PREFIX_BYTES],
    bootstrap: &b::MemberBootstrap<'_>,
    receipt: &b::PortableAdmissionReceipt,
    actual: &c::HostAdvertisement,
    source: &[u8],
    pins: &[b::ContinuationBundleEntry<'_>],
    media: &mut Media,
) {
    let mut scratch = [0; a::PREFIX_BYTES];
    // A genuine committed Root prefix may become obsolete after a checked
    // firmware/Seed replacement. Cold refusal must retain those exact bytes
    // while leaving the normal control protocol and its authority checks up.
    use conduit_mcu_host_runtime::continuation_startup::{self, Startup};
    let prefix_hash = Sha256::digest(selected);
    let media_before_startup = media.bytes.clone();
    let programs_before_startup = media.programs;
    for mismatch in [
        "owner",
        "image",
        "configuration",
        "source",
        "damaged",
        "uncertain",
    ] {
        let mut startup_bootstrap = serde_json::to_value(bootstrap).unwrap();
        startup_bootstrap["admission"] =
            serde_json::to_value(b::MemberBootstrapMaterial::Retained {
                credential: receipt.credential.clone(),
                secret: [3; 32],
            })
            .unwrap();
        let mut configuration = config();
        match mismatch {
            "owner" => {
                startup_bootstrap["owner"]["verifying_key"] = serde_json::to_value(
                    SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes(),
                )
                .unwrap()
            }
            "image" => {
                configuration.image_id = "replacement-image";
                startup_bootstrap["image_id"] = "replacement-image".into();
            }
            "configuration" => {
                configuration.configuration_id = "replacement-config";
                startup_bootstrap["configuration_id"] = "replacement-config".into();
            }
            _ => (),
        }
        let mut startup_carrier = b"CONDUIT_SEED@2\0\0".to_vec();
        let json = serde_json::to_vec(&startup_bootstrap).unwrap();
        startup_carrier.extend_from_slice(&(json.len() as u32).to_le_bytes());
        startup_carrier.extend_from_slice(&json);
        let mut current_actual = actual.clone();
        current_actual.profile = configuration.configuration_id.into();
        current_actual.boot_id = "local/recovery".into();
        let current_seed = Seed::parse(
            &startup_carrier,
            current_actual.host_id.as_str(),
            configuration.image_id,
            configuration.configuration_id,
        )
        .unwrap();
        let mut current = Root::<24576>::new(current_seed, current_actual, configuration).unwrap();
        let mut stale_prefix = selected;
        if mismatch == "damaged" {
            stale_prefix[20] ^= 1;
        }
        if mismatch == "uncertain" {
            stale_prefix[a::PREFIX_BYTES - 1] ^= 1;
        }
        if matches!(mismatch, "image" | "configuration" | "source") {
            // Acceptance alone is insufficient: the SAME issuer's old
            // permission is rejected by original current-Boot IMAGE/Source
            // verification before restore, reservation or a Play attempt.
            assert!(current
                .read_continuation_acceptance(&stale_prefix, &mut scratch)
                .is_ok());
        }
        let source_digest = if mismatch == "source" {
            [9; 32]
        } else {
            Sha256::digest(source).into()
        };
        let startup = Startup::classify(Some(&stale_prefix), |prefix| {
            continuation_startup::verify(&mut current, prefix, &mut scratch, source_digest, pins)
        });
        assert!(startup.accepted().is_none(), "{mismatch}");
        let expected_refusal = match mismatch {
            "owner" | "damaged" => ContinuationAcceptanceRefusal::Signature,
            "uncertain" => ContinuationAcceptanceRefusal::Uncertain,
            _ => ContinuationAcceptanceRefusal::Scope,
        };
        assert_eq!(startup.refusal(), Some(expected_refusal), "{mismatch}");
        assert!(core::ptr::eq(startup.prefix().unwrap(), &stale_prefix));
        assert_eq!(startup.prefix().unwrap(), &stale_prefix);
        assert!(!current.has_current_member());
        assert!(!current.retained_input());
        assert!(current.receipt_digest().is_none());
        assert!(matches!(current.execution_parts(), Err(Refusal::NotMember)));
        assert_eq!(
            current.return_context(1).unwrap().member,
            &receipt.credential
        );
        let mut control = Messages(
            [
                b"?".to_vec(),
                b::PICO_MEMBER_RETURN_IMAGES_REQUEST.to_vec(),
                b::HOST_PREPARATION_REQUEST.to_vec(),
            ]
            .into(),
        );
        assert!(matches!(
            ready(current.receive_request(&mut control)),
            Ok(Request::Inspect)
        ));
        assert!(matches!(
            ready(current.receive_request(&mut control)),
            Ok(Request::Return)
        ));
        assert!(matches!(
            ready(current.receive_request(&mut control)),
            Ok(Request::Prepare)
        ));
        assert!(!current.has_current_member());
        assert!(!current.retained_input());
    }
    assert_eq!(Sha256::digest(selected), prefix_hash);
    assert_eq!(media.bytes, media_before_startup);
    assert_eq!(
        media.programs, programs_before_startup,
        "startup never rewrites rejected media/history"
    );
    let json = serde_json::to_vec(bootstrap).unwrap();
    let mut carrier = b"CONDUIT_SEED@2\0\0".to_vec();
    carrier.extend_from_slice(&(json.len() as u32).to_le_bytes());
    carrier.extend_from_slice(&json);
    let valid_seed = Seed::parse(&carrier, actual.host_id.as_str(), "image", "config").unwrap();
    let mut valid = Root::<24576>::new(valid_seed, actual.clone(), config()).unwrap();
    let startup = Startup::classify(Some(&selected), |prefix| {
        continuation_startup::verify(
            &mut valid,
            prefix,
            &mut scratch,
            Sha256::digest(source).into(),
            pins,
        )
    });
    assert!(startup.accepted().is_some());
    assert!(startup.refusal().is_none());
    assert!(!valid.has_current_member());
    assert_eq!(valid.return_context(1).unwrap().member, &receipt.credential);
    let absent = Startup::classify(None, |_| panic!("absent prefix must not invoke acceptance"));
    assert!(absent.prefix().is_none());
    assert!(absent.accepted().is_none());
    assert!(absent.refusal().is_none());
}
