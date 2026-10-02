use super::*;
use conduit_core::{AuthorityBinding, ConfigurationEntry, ResourceBinding};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[cfg(unix)]
fn fixture() -> (Fixture, EspeakDiscovery) {
    use std::os::unix::{fs::symlink, fs::PermissionsExt};
    let root = std::env::temp_dir().join(format!(
        "conduit-espeak-provider-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let data = root.join("espeak-ng-data");
    fs::create_dir(&data).unwrap();
    fs::write(data.join("voice"), b"reviewed voice bytes").unwrap();
    let executable = root.join("espeak");
    fs::write(&executable, b"fixture executable, never launched").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    let engine = root.join("libespeak-ng.so.1.0");
    fs::write(&engine, b"fixture engine").unwrap();
    symlink("libespeak-ng.so.1.0", root.join("libespeak-ng.so.1")).unwrap();
    let discovery = EspeakDiscovery::inspect(&executable, &data, "en-us", &[engine]).unwrap();
    (Fixture(root), discovery)
}
fn adapter(discovery: EspeakDiscovery) -> EspeakSpeechAdapter {
    discovery
        .initialize(
            "host/speech".into(),
            "boot/speech".into(),
            OfferGeneration(1),
            "grant/speech".into(),
            Duration::from_secs(5),
        )
        .unwrap()
}
fn placement(adapter: &EspeakSpeechAdapter) -> PlannedGear {
    let offer = adapter.offer();
    let resource = adapter.resource_offer();
    let grant = adapter.authority_grant();
    conduit_core::planned_gear_from_parts! {
        placement_id: "placement/speech".into(),
        gear_id: "speech".into(),
        kind_id: offer.kind_id,
        kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry {
            key: "maximum-output-bytes".into(),
            value: ConfigurationValue::U64(u64::from(conduit_tongues::MAXIMUM_PCM_BYTES)),
        }],
        host_id: adapter.host.clone(),
        boot_id: adapter.boot.clone(),
        offer_generation: adapter.generation,
        capability_id: offer.capability_id,
        implementation_id: offer.implementation.implementation_id,
        artifact_id: offer.implementation.artifact_id,
        base: None,
        realization_characteristics: vec![],
        limits: offer.limits,
        inputs: offer.inputs,
        outputs: offer.outputs,
        semantic_contract: offer.semantic_contract,
        terminal_transductions: vec![],
        host_calls: offer.host_calls,
        resources: vec![ResourceBinding {
            pool_id: resource.pool_id,
            class_id: resource.class_id,
            units: 1,
            protected: None,
            compute: None,
            content: resource.content,
        }],
        authority: vec![AuthorityBinding {
            grant_id: grant.grant_id,
            contract_id: grant.contract_id,
            host_call_contract_id: grant.host_call_contract_id,
            subject_kind: grant.subject_kind,
            host_id: grant.host_id,
            boot_id: grant.boot_id,
            capability_id: grant.capability_id,
        }],
        pool_references: vec![],
    }
}
#[test]
#[cfg(unix)]
fn exact_provider_admission_rejects_forged_placement_and_changed_content() {
    let (files, discovery) = fixture();
    let adapter = adapter(discovery);
    let original = placement(&adapter);
    assert_eq!(
        adapter.validate_placement(&original),
        Ok(conduit_tongues::MAXIMUM_PCM_BYTES)
    );
    let mut stale = original.clone();
    stale.offer_generation = OfferGeneration(2);
    assert_eq!(
        adapter.validate_placement(&stale),
        Err(EspeakFailure::StaleProvider)
    );
    let mut forged = original.clone();
    forged.authority[0].grant_id = "grant/other".into();
    assert_eq!(
        adapter.validate_placement(&forged),
        Err(EspeakFailure::WrongAuthority)
    );
    forged = original.clone();
    forged.resources[0].pool_id = "pool/other".into();
    assert_eq!(
        adapter.validate_placement(&forged),
        Err(EspeakFailure::WrongResource)
    );
    forged = original;
    forged.implementation_id = "implementation/other".into();
    assert_eq!(
        adapter.validate_placement(&forged),
        Err(EspeakFailure::WrongPlacement)
    );
    fs::write(files.0.join("espeak-ng-data/voice"), b"changed voice").unwrap();
    assert_eq!(
        adapter.discovery.verify(),
        Err(EspeakFailure::ProviderChanged)
    );
}
#[test]
#[cfg(unix)]
fn discovery_refuses_symlinks_and_engine_replacement() {
    let (files, discovery) = fixture();
    std::os::unix::fs::symlink("/etc/passwd", files.0.join("espeak-ng-data/escape")).unwrap();
    assert_eq!(discovery.verify(), Err(EspeakFailure::ProviderChanged));
    fs::remove_file(files.0.join("espeak-ng-data/escape")).unwrap();
    fs::write(&discovery.engine, b"changed engine").unwrap();
    assert_eq!(discovery.verify(), Err(EspeakFailure::ProviderChanged));
}
fn wave() -> Vec<u8> {
    let mut bytes = b"RIFF\x28\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x22\x56\0\0\x44\xac\0\0\x02\0\x10\0data\x04\0\0\0\x01\0\x02\0".to_vec();
    bytes[4..8].copy_from_slice(&40_u32.to_le_bytes());
    bytes
}
#[test]
fn wav_requires_exact_bounded_mono_pcm_or_espeak_streaming_header() {
    let mut bytes = wave();
    assert_eq!(wav::pcm(&bytes, 4).unwrap(), [1, 0, 2, 0]);
    assert_eq!(wav::pcm(&bytes, 2), Err(EspeakFailure::OutputOverflow));
    bytes[4..8].copy_from_slice(&0x7ffff024_u32.to_le_bytes());
    bytes[40..44].copy_from_slice(&0x7ffff000_u32.to_le_bytes());
    assert!(wav::pcm(&bytes, 4).is_ok());
    bytes[22] = 2;
    assert_eq!(wav::pcm(&bytes, 4), Err(EspeakFailure::InvalidWav));
    assert_eq!(wav::pcm(b"not WAV", 4), Err(EspeakFailure::InvalidWav));
}
#[test]
#[cfg(unix)]
fn text_is_stdin_only_and_cancelled_request_never_launches() {
    let (_files, discovery) = fixture();
    let adapter = adapter(discovery);
    let placement = placement(&adapter);
    assert!(!adapter.arguments.iter().any(|argument| argument == "-m"));
    let mut pcm = Vec::with_capacity(conduit_tongues::MAXIMUM_PCM_BYTES as usize);
    assert_eq!(
        adapter.synthesize_into(&placement, b"--help", &mut pcm, || true),
        Err(EspeakFailure::Cancelled)
    );
    assert!(pcm.is_empty());
    assert_eq!(
        adapter.synthesize_into(&placement, &[b'a'; 257], &mut pcm, || false),
        Err(EspeakFailure::InvalidText)
    );
    assert_eq!(
        adapter.synthesize_into(&placement, b"a\0b", &mut pcm, || false),
        Err(EspeakFailure::InvalidText)
    );
}
#[test]
fn failures_keep_authority_cancellation_and_capacity_distinct() {
    use conduit_kernel::{FailureCode, HostCallDisposition};
    assert_eq!(
        EspeakFailure::WrongAuthority.host_failure().0,
        HostCallDisposition::Denied
    );
    assert_eq!(
        EspeakFailure::Cancelled.host_failure().0,
        HostCallDisposition::Cancelled
    );
    assert_eq!(
        EspeakFailure::OutputOverflow.host_failure().1.code,
        FailureCode::WorkBudgetExhausted
    );
    assert_ne!(
        EspeakFailure::Timeout.host_failure().1.detail,
        EspeakFailure::ProviderLost.host_failure().1.detail
    );
}
#[test]
#[ignore = "requires explicitly selected installed eSpeak NG and English voice data; creates audio bytes but never playback"]
fn installed_espeak_produces_real_bounded_pcm_from_parameter_like_text() {
    let engine = fs::canonicalize("/usr/lib/x86_64-linux-gnu/libespeak-ng.so.1").unwrap();
    let discovery = EspeakDiscovery::inspect(
        Path::new("/usr/bin/espeak-ng"),
        Path::new("/usr/lib/x86_64-linux-gnu/espeak-ng-data"),
        "en-us",
        &[engine],
    )
    .unwrap();
    let adapter = adapter(discovery);
    let placement = placement(&adapter);
    let mut pcm = Vec::with_capacity(conduit_tongues::MAXIMUM_PCM_BYTES as usize);
    let receipt = adapter
        .synthesize_into(&placement, b"--help", &mut pcm, || false)
        .unwrap();
    assert!(receipt.pcm_bytes > 1000);
    assert!(pcm.iter().any(|byte| *byte != 0));
    assert_eq!(receipt.pcm_bytes as usize, pcm.len());
    assert_eq!(
        receipt.text_sha256,
        format!("{:x}", Sha256::digest(b"--help"))
    );
}
