use super::*;
fn maximum_profiles() -> KnownNetworkProfiles {
    let mut profiles = KnownNetworkProfiles::new();
    for (index, priority) in [-128, -1, 0, 127].into_iter().enumerate() {
        let mut ssid = [b'x'; 32];
        ssid[0] = b'a' + index as u8;
        profiles
            .upsert(
                PrivateNetworkInput::new(
                    &ssid,
                    &[b'k'; 63],
                    if index % 2 == 0 {
                        KnownSecurity::Wpa2Personal
                    } else {
                        KnownSecurity::Wpa3Personal
                    },
                    priority,
                )
                .unwrap(),
            )
            .unwrap();
    }
    profiles
}
#[test]
fn four_maximum_profiles_exact_private_roundtrip_security_priority_and_capacity() {
    assert_eq!(MAXIMUM_KNOWN_NETWORK_PRIVATE_BYTES, 401);
    let profiles = maximum_profiles();
    let mut bytes = [0; MAXIMUM_KNOWN_NETWORK_PRIVATE_BYTES];
    let length = encode_private_known_networks(&profiles, &mut bytes).unwrap();
    assert_eq!(length, 401);
    assert_eq!(&bytes[..5], b"CKN\x01\x04");
    assert_eq!(&bytes[5..9], &[1, 128, 32, 63]);
    assert_eq!(&bytes[104..108], &[2, 255, 32, 63]);
    assert_eq!(&bytes[302..306], &[2, 127, 32, 63]);
    let restored = decode_private_known_networks(&bytes).unwrap();
    assert_eq!(restored.len(), 4);
    for (original, restored_info) in profiles.entries().zip(restored.entries()) {
        assert_eq!(original.security, restored_info.security);
        assert_eq!(original.priority, restored_info.priority);
        profiles
            .with_request(original.key, |a| {
                restored
                    .with_request(restored_info.key, |b| {
                        assert_eq!(a.ssid, b.ssid);
                        assert_eq!(a.credential, b.credential);
                    })
                    .unwrap()
            })
            .unwrap();
    }
    let mut second = [0; 401];
    assert_eq!(
        encode_private_known_networks(&restored, &mut second).unwrap(),
        401
    );
    assert_eq!(bytes, second);
    let mut too_small = [0xa5; 400];
    assert_eq!(
        encode_private_known_networks(&profiles, &mut too_small),
        Err(KnownNetworkWireRefusal::Capacity)
    );
    assert_eq!(too_small, [0xa5; 400]);
    let mut empty = [0; 5];
    assert_eq!(
        encode_private_known_networks(&KnownNetworkProfiles::new(), &mut empty).unwrap(),
        5
    );
    assert!(decode_private_known_networks(&empty).unwrap().is_empty());
}
#[test]
fn every_truncation_duplicates_unknown_fields_and_trailing_data_refuse_atomically() {
    let profiles = maximum_profiles();
    let mut bytes = [0; 401];
    encode_private_known_networks(&profiles, &mut bytes).unwrap();
    for cut in 0..bytes.len() {
        assert!(
            decode_private_known_networks(&bytes[..cut]).is_err(),
            "cut {cut}"
        );
    }
    for (at, value) in [
        (0, b'X'),
        (3, 2),
        (4, 5),
        (5, 0),
        (7, 0),
        (7, 33),
        (8, 7),
        (8, 64),
        (9, 0),
        (9, 0xff),
        (41, 0),
    ] {
        let mut corrupt = bytes;
        corrupt[at] = value;
        assert!(
            decode_private_known_networks(&corrupt).is_err(),
            "field {at}"
        );
    }
    let mut duplicate = bytes;
    duplicate[108..140].copy_from_slice(&bytes[9..41]);
    assert!(matches!(
        decode_private_known_networks(&duplicate),
        Err(KnownNetworkWireRefusal::DuplicateSsid)
    ));
    let mut trailing = [0; 402];
    trailing[..401].copy_from_slice(&bytes);
    assert!(decode_private_known_networks(&trailing).is_err());
    let mut count_mismatch = bytes;
    count_mismatch[4] = 3;
    assert!(matches!(
        decode_private_known_networks(&count_mismatch),
        Err(KnownNetworkWireRefusal::Trailing)
    ));
    // A decode refusal cannot mutate the independently retained source set.
    let mut unchanged = [0; 401];
    encode_private_known_networks(&profiles, &mut unchanged).unwrap();
    assert_eq!(unchanged, bytes);
}
#[test]
fn legacy_migration_uses_only_exact_canonical_cnj1_and_never_unknown_magic() {
    let mut legacy = [0; 167];
    let length = crate::encode_network_join_request(
        crate::NetworkJoinRequest {
            ssid: b"legacy",
            credential: b"public-test-pass",
        },
        &mut legacy,
    )
    .unwrap();
    let restored = decode_private_known_networks_or_legacy(&legacy[..length]).unwrap();
    let info = restored.entries().next().unwrap();
    assert_eq!(info.security, KnownSecurity::Wpa2Personal);
    assert_eq!(info.priority, 0);
    restored
        .with_request(info.key, |r| {
            assert_eq!(r.ssid, b"legacy");
            assert_eq!(r.credential, b"public-test-pass");
        })
        .unwrap();
    assert!(migrate_private_legacy_network(b"garbage").is_err());
    assert!(decode_private_known_networks_or_legacy(b"unknown format").is_err());
    for cut in 0..length {
        assert!(migrate_private_legacy_network(&legacy[..cut]).is_err());
    }
    for at in [0, 4, 5, 6] {
        let mut wrong = legacy;
        wrong[at] = 0xff;
        assert!(decode_private_known_networks_or_legacy(&wrong[..length]).is_err());
    }
    assert!(migrate_private_legacy_network(&legacy[..length + 1]).is_err());
    let mut modern = [0; 401];
    let size = encode_private_known_networks(&restored, &mut modern).unwrap();
    assert_eq!(&modern[..3], b"CKN");
    assert_eq!(
        decode_private_known_networks_or_legacy(&modern[..size])
            .unwrap()
            .len(),
        1
    );
}
