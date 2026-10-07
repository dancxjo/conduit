#![cfg(feature = "kernel-step")]
use conduit_ai::fixed_numeric_catalog::*;
const SELECTED_FRAME_BYTES: usize = 16_384;
use conduit_plot::*;
#[test]
fn all_explicit_phase_and_final_pcm16_anchor_records_fit_exact_transport_envelopes() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let source = [
        include_str!("../../speech/fargan_conditioning.conduit"),
        include_str!("../../speech/fargan_signal.conduit"),
        include_str!("../../speech/fargan_pitch_history.conduit"),
        include_str!("../../speech/fargan_subframe.conduit"),
        include_str!("../../speech/fargan_model_identity.conduit"),
        include_str!("../../speech/fargan_epoch_contracts.conduit"),
    ]
    .join("\n");
    conduit_ai::fixed_numeric_pair_catalog::install_fixed_numeric_pair_catalogs(
        &mut startup,
        &mut profiles,
    )
    .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    for name in [
        "FarganFloatEpochInput",
        "FarganFloatPhase1",
        "FarganFloatPhase2",
        "FarganFloatPhase3",
        "FarganFloatEpochProposal",
        "FarganPcm16EpochResult",
    ] {
        let ty = &checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type;
        let maximum = maximum_prepared_transport_value_bytes(ty).unwrap();
        eprintln!("{name}: max canonical transport {maximum}");
        assert!(
            maximum as usize <= SELECTED_FRAME_BYTES,
            "{name}: {maximum}"
        );
    }
    let right = &checked
        .native_types
        .iter()
        .find(|t| t.name == "FarganSubframeResult")
        .unwrap()
        .value_type;
    let right_max = maximum_prepared_canonical_value_bytes(right).unwrap();
    {
        let name = "FarganEpochPhaseCarry";
        let left = &checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .value_type;
        let left_max = maximum_prepared_canonical_value_bytes(left).unwrap();
        let pair = conduit_core::PreparedTypedTuplePairEncoder::new(
            left.clone(),
            left_max,
            right.clone(),
            right_max,
        )
        .unwrap();
        eprintln!(
            "{name}: left={left_max}, right={right_max}, paired={}",
            pair.maximum_bytes()
        );
        assert!(pair.maximum_bytes() as usize <= SELECTED_FRAME_BYTES);
    }
}
