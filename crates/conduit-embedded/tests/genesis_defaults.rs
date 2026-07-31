use conduit_core::{
    DISTRIBUTION_PROFILE_SCHEMA_VERSION, DistributionProvider, GenesisReason, HostDistributionKind,
    Id, PinnedDescriptor, ProviderAvailability, ProviderRequirement, ProviderRiskTraits,
    ReferenceDistributionProfile, SemanticHash, require_provider, validate_reference_distribution,
};

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);

const fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

const FIRMWARE_TRAITS: ProviderRiskTraits = ProviderRiskTraits {
    enrollment_issuer: false,
    unrestricted_native_execution: false,
    remote_artifact_installation: false,
    firmware_mutation: true,
    unrestricted_network: false,
    realm_root_administration: false,
    remote_plan_activation: false,
    actuating_effects: false,
};

static PROVIDERS: [DistributionProvider<'static>; 2] = [
    DistributionProvider {
        provider: pin("provider/firmware-mutation", 1),
        artifact: None,
        availability: ProviderAvailability::Unsupported,
        traits: FIRMWARE_TRAITS,
    },
    DistributionProvider {
        provider: pin("provider/bounded-embedded-runtime", 2),
        artifact: None,
        availability: ProviderAvailability::Enabled,
        traits: ProviderRiskTraits {
            enrollment_issuer: false,
            unrestricted_native_execution: false,
            remote_artifact_installation: false,
            firmware_mutation: false,
            unrestricted_network: false,
            realm_root_administration: false,
            remote_plan_activation: false,
            actuating_effects: false,
        },
    },
];

fn constrained_profile() -> ReferenceDistributionProfile<'static> {
    let mut profile = ReferenceDistributionProfile {
        schema_version: DISTRIBUTION_PROFILE_SCHEMA_VERSION,
        identity: ZERO,
        descriptor: pin("distribution/reference-constrained", 3),
        kind: HostDistributionKind::Constrained,
        genesis_profile: hash(4),
        control_recorder: pin("recorder/genesis-control", 5),
        provider_enablement_effect_class: pin("effect/provider-enablement", 6),
        provider_enablement_operation: pin("operation/provider-enable", 7),
        providers: &PROVIDERS,
        maximum_provider_enablement_ticks: 30,
        maximum_provider_install_attempts: 1,
        maximum_evidence_events: 16,
    };
    let mut scratch = [ZERO; 2];
    profile.identity = profile.computed_semantic_hash(&mut scratch).unwrap();
    profile
}

#[test]
fn constrained_reference_profile_fails_closed_without_firmware_provider() {
    let profile = constrained_profile();
    let mut scratch = [ZERO; 2];
    assert_eq!(
        validate_reference_distribution(profile, &mut scratch),
        Ok(())
    );
    assert_eq!(
        require_provider(
            profile,
            ProviderRequirement {
                provider: PROVIDERS[0].provider,
                traits: FIRMWARE_TRAITS,
            },
        ),
        Err(GenesisReason::ProviderUnavailable)
    );
    assert!(
        require_provider(
            profile,
            ProviderRequirement {
                provider: PROVIDERS[1].provider,
                traits: ProviderRiskTraits::default(),
            },
        )
        .is_ok()
    );
}
