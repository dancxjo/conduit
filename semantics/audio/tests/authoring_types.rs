use conduit_audio::{authoring, *};
use conduit_core::Unit;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

fn program(input: &str, output: &str, expression: &str) -> PortableExpressionProgram {
    let mut startup = StartupCatalog::new();
    authoring::install(&mut startup).unwrap();
    let source = format!(
        "plot imported/construct (\n >> value: {input}\n result: {output} >>\n) = ({expression})"
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "imported/construct", &ProfileCatalog::new())
            .unwrap();
    let conduit_core::ConfigurationValue::Text(encoded) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("portable expression program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

#[test]
fn imported_types_retain_native_abi_and_complete_owner_laws() {
    let types = authoring::checked_types().unwrap();
    macro_rules! same {
        ($($ty:ident),+ $(,)?) => {$(
            let ty = types.iter().find(|ty| ty.name == stringify!($ty)).unwrap();
            let native = $ty::semantic_type().unwrap();
            assert_eq!(ty.value_type, native);
            let schema = match native.shape() {
                conduit_core::StructuredInfoTypeShape::Nominal { schema, .. } => schema,
                conduit_core::StructuredInfoTypeShape::Record { schema, .. }
                | conduit_core::StructuredInfoTypeShape::Variant { schema, .. } => schema,
                _ => panic!("owned Audio Type must retain a nominal identity"),
            };
            assert_eq!(&ty.identity, schema);
        )+};
    }
    same!(
        AudioFrequencyHz,
        AudioCycleDuration,
        AudioRelativeAmplitude,
        AudioPowerRatio,
        AudioTrajectoryAnchor,
        AudioExactTimeOffset,
        AudioDecibelBasis,
        AudioReferencedLevelRatio,
    );
    let basis = types
        .iter()
        .find(|ty| ty.name == "AudioDecibelBasis")
        .unwrap();
    assert!(!basis.invariants.is_empty());
    let hz = types
        .iter()
        .find(|ty| ty.name == "AudioFrequencyHz")
        .unwrap();
    assert!(!hz.value_contracts.is_empty());
}

#[test]
fn authored_construction_keeps_imported_denominator_refinement() {
    let construct = program(
        "AudioFrequencyHz",
        "AudioFrequencyHz",
        "{numerator_hz: .numerator_hz, denominator: .denominator - 1}",
    );
    assert!(construct
        .evaluate(&AudioFrequencyHz::new(1, 200).unwrap().encode().unwrap())
        .is_err());
    let valid = construct
        .evaluate(&AudioFrequencyHz::new(2, 200).unwrap().encode().unwrap())
        .unwrap();
    assert_eq!(
        AudioFrequencyHz::decode(&valid).unwrap(),
        AudioFrequencyHz::new(1, 200).unwrap()
    );
}

#[test]
fn authored_construction_keeps_imported_reference_role_convention_law() {
    let reference = AudioDecibelReference::new(
        1,
        "declared".into(),
        1,
        AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Authored,
            "test declaration".into(),
            None,
        )
        .unwrap(),
        AudioDecibelReferenceRole::Power,
        Unit::Second,
    )
    .unwrap();
    let wrong = program(
        "AudioDecibelReference",
        "AudioDecibelBasis",
        "{reference: ., convention: AudioDecibelConvention.amplitude_twenty_log10}",
    );
    assert!(wrong
        .evaluate(&reference.clone().encode().unwrap())
        .is_err());
    let valid = program(
        "AudioDecibelReference",
        "AudioDecibelBasis",
        "{reference: ., convention: AudioDecibelConvention.power_ten_log10}",
    )
    .evaluate(&reference.encode().unwrap())
    .unwrap();
    assert_eq!(
        AudioDecibelBasis::decode(&valid).unwrap().convention(),
        &AudioDecibelConvention::PowerTenLog10
    );
}
