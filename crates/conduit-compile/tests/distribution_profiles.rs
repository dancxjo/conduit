use conduit_compile::ReferenceDistributionDocument;

const PROFILES: [(&str, &str); 3] = [
    (
        "hosted",
        include_str!("../../../distribution/reference-hosted-v1.json"),
    ),
    (
        "browser",
        include_str!("../../../distribution/reference-browser-v1.json"),
    ),
    (
        "constrained",
        include_str!("../../../distribution/reference-constrained-v1.json"),
    ),
];

#[test]
fn checked_in_reference_distributions_are_sealed_safe_defaults() {
    for (kind, source) in PROFILES {
        let document: ReferenceDistributionDocument = serde_json::from_str(source).unwrap();
        assert_eq!(document.kind, kind);
        assert!(document.requirements.is_empty());
        assert!(
            document
                .providers
                .iter()
                .filter(|provider| provider.traits != Default::default())
                .all(|provider| provider.availability != "enabled")
        );
        let mut sealed = document.clone();
        sealed.seal().unwrap();
        assert_eq!(document.identity, sealed.identity, "{kind}");
        document.validate().unwrap();
    }
}

#[test]
fn browser_and_constrained_profiles_report_dangerous_features_as_unsupported() {
    for (kind, source) in PROFILES.into_iter().skip(1) {
        let document: ReferenceDistributionDocument = serde_json::from_str(source).unwrap();
        for provider in &document.providers {
            if provider.traits.enrollment_issuer
                || provider.traits.unrestricted_native_execution
                || provider.traits.remote_artifact_installation
                || provider.traits.firmware_mutation
                || provider.traits.realm_root_administration
                || provider.traits.remote_plan_activation
                || provider.traits.actuating_effects
            {
                assert_eq!(provider.availability, "unsupported", "{kind}");
            }
        }
    }
}
