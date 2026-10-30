#![cfg(target_os = "linux")]
//! Real fresh Host constructors and provider initialization over an explicit
//! discovery executable fixture. No hardware capture, native Root admission,
//! resource grant, protected Line, Plan or Play is established by this test.
use conduit_body::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, Body, BodyMembership,
    PortableSpawnAdmissionRequest, RoutedAdmissionRequest, SpawnInvitationSecret,
    COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA, SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::{BaseLifecycle, HostAdvertisement, OfferGeneration};
use conduit_std_host::{
    hosted_microphone::{AlsaMicrophoneDiscovery, MicrophoneLimits},
    StdHost, StdHostComposition, StdHostConfig,
};
use std::{os::unix::fs::PermissionsExt, time::Duration};

struct Initialized {
    host: StdHost,
    directory: tempfile::TempDir,
    provider_sha256: String,
}
fn initialized(composition: StdHostComposition) -> Initialized {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("fixture-arecord");
    // Only ALSA control-list discovery is accepted. Capture would leave a marker
    // and fail, so this test cannot quietly become hardware/audio-I/O evidence.
    std::fs::write(&executable, "#!/bin/sh\nif [ \"$2\" = -l ]; then printf 'card 999: Fixture [Fixture], device 0: Capture [Capture]\\n'; exit 0; fi\ntouch \"$(dirname \"$0\")/capture-attempted\"\nexit 97\n").unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let discovery = AlsaMicrophoneDiscovery::inspect(&executable).unwrap();
    assert_eq!(discovery.observations.len(), 1);
    let observation = discovery.observations[0].clone();
    let provider_sha256 = discovery.executable_sha256.clone();
    let adapter = discovery
        .initialize(
            &observation,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    // Selection precedes advertisement and ledger construction, never filters
    // an advertisement produced by the reference/universal constructor.
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: "host/purpose-admission-fixture".into(),
            boot_id: "boot/purpose-admission-fixture".into(),
            offer_generation: OfferGeneration(1),
        },
        composition,
    );
    assert!(host.advertisement().bases.is_empty());
    host.attach_microphone(adapter).unwrap();
    Initialized {
        host,
        directory,
        provider_sha256,
    }
}
fn verify_initialized(fixture: &Initialized) {
    let ad = fixture.host.advertisement();
    assert_eq!(ad.host_id.as_str(), "host/purpose-admission-fixture");
    assert_eq!(ad.boot_id.as_str(), "boot/purpose-admission-fixture");
    assert_eq!(ad.offer_generation, OfferGeneration(2));
    assert_eq!(ad.bases.len(), 1);
    let base = &ad.bases[0];
    assert_eq!(base.lifecycle, BaseLifecycle::Ready);
    assert_eq!(base.provider_generation, ad.offer_generation.0);
    assert!(base
        .provider_instance_id
        .as_str()
        .contains(&fixture.provider_sha256));
    assert_eq!(
        base.implementation_id.as_str(),
        conduit_std_offers::MICROPHONE_CLIP_IMPLEMENTATION
    );
    assert_eq!(base.capability_ids.len(), 1);
    assert_eq!(base.resource_pool_ids.len(), 1);
    let capability = ad
        .capabilities
        .iter()
        .find(|c| c.capability_id == base.capability_ids[0])
        .unwrap();
    assert_eq!(
        capability.implementation.implementation_id.as_str(),
        base.implementation_id.as_str()
    );
    assert!(
        !capability.authority_requirements.is_empty(),
        "initialization must not erase capture authority requirements"
    );
    let resource = ad
        .resources
        .iter()
        .find(|r| r.pool_id == base.resource_pool_ids[0])
        .unwrap();
    assert_eq!(
        resource.class_id.as_str(),
        conduit_std_offers::MICROPHONE_CAPTURE_RESOURCE_CLASS
    );
    assert_eq!(resource.capacity_units, 1);
    assert!(!fixture.directory.path().join("capture-attempted").exists());
}
fn signs() -> AdmissionSigns {
    AdmissionSigns {
        part_admitted: "sign/purpose/part".into(),
        host_attached: "sign/purpose/host".into(),
        candidate_admitted: "sign/purpose/candidate".into(),
    }
}
fn routed(
    advertisement: &HostAdvertisement,
) -> (RoutedAdmissionRequest, AdmissionManager, BodyMembership) {
    let body = Body::born(
        "source/purpose-codec-fixture".into(),
        "checked/purpose-codec-fixture".into(),
        1,
        "sign/purpose/born".into(),
    )
    .unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    // Explicit CPU invitation/signature fixture, not a provisioned Body secret.
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap(),
            [12; 32],
            1000,
            2000,
        )
        .unwrap();
    let signature = invitation.secret.sign(&invitation.signing_transcript(
        &advertisement.host_id,
        &advertisement.boot_id,
        advertisement.offer_generation,
    ));
    let request = RoutedAdmissionRequest {
        schema: COMPACT_ROUTED_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.invitation_id.as_str().into(),
        request: PortableSpawnAdmissionRequest {
            schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: invitation.invitation_id.clone(),
            body_id: invitation.body_id.clone(),
            // The entire current advertisement, unfiltered and unchanged.
            host_advertisement: advertisement.clone(),
            nonce: invitation.nonce,
            signature: signature.to_vec(),
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        },
    };
    (request, manager, BodyMembership::new(body.body_id).unwrap())
}
fn roundtrip(name: &str, ad: &HostAdvertisement) -> RoutedAdmissionRequest {
    let (request, _, _) = routed(ad);
    let expanded = serde_json::to_vec(ad).unwrap();
    let encoded = serde_json::to_vec(&request).unwrap_or_else(|error| {
        panic!(
            "complete {name} advertisement refused by unchanged v2 codec: {error}; expanded={}B",
            expanded.len()
        )
    });
    let envelope: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    let field = &envelope["request"]["host_advertisement"];
    assert_eq!(
        field["expanded_bytes"].as_u64().unwrap(),
        expanded.len() as u64
    );
    let compressed = field["zlib_hex"].as_str().unwrap().len() / 2;
    assert!(expanded.len() <= 64 * 1024);
    assert!(compressed <= 24 * 1024);
    assert!(encoded.len() <= 48 * 1024, "complete native carrier bound");
    let decoded: RoutedAdmissionRequest = serde_json::from_slice(&encoded).unwrap();
    assert!(decoded.supported_schema());
    assert_eq!(decoded.request.host_advertisement, *ad);
    assert_eq!(decoded.request.signature, request.request.signature);
    assert!(
        !decoded.request.membership_admitted
            && !decoded.request.plan_created
            && !decoded.request.play_created
    );
    eprintln!("purpose-admission {name}: capabilities={} bases={} resources={} expanded={} compressed={} routed={} bytes; CPU fixture only",ad.capabilities.len(),ad.bases.len(),ad.resources.len(),expanded.len(),compressed,encoded.len());
    decoded
}
#[test]
fn complete_initialized_microphone_host_roundtrips_without_unrelated_input_or_speech() {
    let name = "microphone";
    let composition = StdHostComposition::minimal();
    let fixture = initialized(composition);
    verify_initialized(&fixture);
    let ad = fixture.host.advertisement();
    assert!(!ad
        .capabilities
        .iter()
        .any(|c| c.kind_id.as_str() == "input/keymap"));
    assert!(!ad.capabilities.iter().any(
        |c| c.kind_id.as_str() == "speech/utterance" || c.kind_id.as_str().starts_with("ipa/")
    ));
    let decoded = roundtrip(name, ad);
    assert_eq!(decoded.request.host_advertisement.bases, ad.bases);
    assert!(!fixture.directory.path().join("capture-attempted").exists());
}
#[test]
fn complete_request_keeps_host_boot_generation_signature_and_replay_checks() {
    let fixture = initialized(StdHostComposition::minimal());
    verify_initialized(&fixture);
    let request = roundtrip("authenticated-microphone", fixture.host.advertisement());
    let (_, mut manager, mut membership) = routed(fixture.host.advertisement());
    for field in ["host", "boot", "generation"] {
        let mut altered = request.request.clone();
        match field {
            "host" => altered.host_advertisement.host_id = "host/foreign".into(),
            "boot" => altered.host_advertisement.boot_id = "boot/foreign".into(),
            _ => altered.host_advertisement.offer_generation = OfferGeneration(3),
        }
        let mut isolated_manager = manager.clone();
        let mut isolated_membership = membership.clone();
        assert!(isolated_manager
            .complete_spawn(
                &mut isolated_membership,
                &altered.host_advertisement,
                &altered.admission_proof().unwrap(),
                1001,
                signs()
            )
            .is_err());
        assert!(isolated_membership.parts.is_empty());
    }
    let credential = manager
        .complete_spawn(
            &mut membership,
            &request.request.host_advertisement,
            &request.request.admission_proof().unwrap(),
            1001,
            signs(),
        )
        .unwrap();
    assert_eq!(credential.host_id, fixture.host.advertisement().host_id);
    assert_eq!(credential.boot_id, fixture.host.advertisement().boot_id);
    assert_eq!(
        manager.complete_spawn(
            &mut membership,
            &request.request.host_advertisement,
            &request.request.admission_proof().unwrap(),
            1002,
            signs()
        ),
        Err(AdmissionRefusal::Replay)
    );
}
#[test]
fn complete_optional_families_keep_visible_codec_refusals() {
    for (name, composition) in [
        ("microphone-time", StdHostComposition::minimal().with_time()),
        ("microphone-text", StdHostComposition::minimal().with_text()),
        (
            "microphone-time-text",
            StdHostComposition::minimal().with_time().with_text(),
        ),
        (
            "microphone-input",
            StdHostComposition::minimal().with_input(),
        ),
        (
            "microphone-native-speech",
            StdHostComposition::minimal().with_native_speech(),
        ),
        ("microphone-reference", StdHostComposition::reference()),
    ] {
        let fixture = initialized(composition);
        verify_initialized(&fixture);
        let ad = fixture.host.advertisement();
        assert_eq!(
            ad.capabilities
                .iter()
                .any(|c| c.kind_id.as_str() == "input/keymap"),
            composition.input
        );
        assert_eq!(
            ad.capabilities
                .iter()
                .any(|c| c.kind_id.as_str() == "speech/utterance"),
            composition.native_speech
        );
        let (request, _, _) = routed(ad);
        let expanded = serde_json::to_vec(ad).unwrap().len();
        match serde_json::to_vec(&request) {
            Ok(encoded) => {
                eprintln!(
                    "purpose-admission {name}: complete expanded={expanded} routed={} bytes",
                    encoded.len()
                );
                let decoded: RoutedAdmissionRequest = serde_json::from_slice(&encoded).unwrap();
                assert_eq!(decoded.request.host_advertisement, *ad);
            }
            Err(error) => {
                eprintln!("purpose-admission {name}: unchanged codec REFUSED complete expanded={expanded} bytes: {error}");
                assert!(
                    expanded > 64 * 1024
                        || error
                            .to_string()
                            .contains("compressed admission advertisement capacity")
                );
            }
        }
        if composition.input {
            assert!(serde_json::to_vec(&request).is_err(),"full required keymap default still exceeds original codec bound; never silently omit it");
        }
        assert!(!fixture.directory.path().join("capture-attempted").exists());
    }
}
