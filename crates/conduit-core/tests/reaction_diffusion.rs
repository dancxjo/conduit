use conduit_core::{
    GrayScottParameters, ReactionDiffusionCell, ReactionDiffusionEvolveRequest,
    ReactionDiffusionFieldId, ReactionDiffusionFieldState, ReactionDiffusionRefusal,
    REACTION_DIFFUSION_MAXIMUM_GENERATIONS,
};

const FIELD_ID: ReactionDiffusionFieldId = ReactionDiffusionFieldId(*b"field-a0-proof01");

#[test]
fn initialized_field_has_stable_encoding_digest_and_golden_generations() {
    let initial = state(8, 8, 17);
    let encoded = initial.encode().unwrap();
    assert_eq!(
        ReactionDiffusionFieldState::decode(&encoded),
        Ok(initial.clone())
    );
    assert_eq!(
        initial.semantic_digest().unwrap(),
        initial.semantic_digest().unwrap()
    );

    let evolved = initial.evolve_reference(request(0, 4, 256)).unwrap();
    assert_eq!(evolved.generation, 4);
    assert_eq!(
        hex(evolved.semantic_digest().unwrap()),
        "a8fe01ee5152510fcfe904d9b32c2f01d3344acb0602e2ff8f82f8eb9baed5bf"
    );
    assert_eq!(
        evolved,
        state(8, 8, 17)
            .evolve_reference(request(0, 2, 128))
            .unwrap()
            .evolve_reference(request(2, 2, 128))
            .unwrap()
    );
}

#[test]
fn three_distinct_seeds_have_byte_exact_multi_generation_goldens() {
    let vectors = [
        (0, "434e44464c443031315053470300030003000000000000006669656c642d61302d70726f6f6630310071020080380100b8880000e8fd000040420f0009000000af530700af2f05007a0e070084d605001cac060057af06008e290700d1a1050076420700ca66050060f00600b11e06007c5607001f360500a00a07007feb0500f620070074ad0500"),
        (17, "434e44464c443031315053470300030003000000000000006669656c642d61302d70726f6f6630310071020080380100b8880000e8fd000040420f00090000003a1f07003f1806006ca1070004f00400d820070004150600f99b0600ec2d07006a3d070033ce0500e39d0600282a0700d1d30600d5bc0600c7680700647305002cd606000cb80600"),
        (99, "434e44464c443031315053470300030003000000000000006669656c642d61302d70726f6f6630310071020080380100b8880000e8fd000040420f0009000000344b0700509005006f380700cfc60500cf4c0700f28c0500521b0700fbfe0500116d070052480500911d070036fa050063950600161807001aff0600523406002198060068120700"),
    ];
    for (seed, expected) in vectors {
        let evolved = state(3, 3, seed)
            .evolve_reference(request(0, 3, 27))
            .unwrap();
        assert_eq!(hex_vec(&evolved.encode().unwrap()), expected);
        assert_eq!(
            evolved.encode().unwrap(),
            state(3, 3, seed)
                .evolve_reference(request(0, 1, 9))
                .unwrap()
                .evolve_reference(request(1, 2, 18))
                .unwrap()
                .encode()
                .unwrap()
        );
    }
}

#[test]
fn malformed_profile_dimensions_parameters_and_cells_refuse_distinctly() {
    assert_eq!(
        ReactionDiffusionFieldState::initialized(FIELD_ID, 2, 8, GrayScottParameters::REFERENCE, 1,),
        Err(ReactionDiffusionRefusal::InvalidDimensions)
    );
    let mut invalid_parameters = GrayScottParameters::REFERENCE;
    invalid_parameters.time_step_ppm = 0;
    assert_eq!(
        ReactionDiffusionFieldState::initialized(FIELD_ID, 8, 8, invalid_parameters, 1),
        Err(ReactionDiffusionRefusal::InvalidParameters)
    );
    assert_eq!(
        ReactionDiffusionCell::new(1_000_001, 0),
        Err(ReactionDiffusionRefusal::ConcentrationOutOfRange)
    );

    let mut wrong_profile = state(8, 8, 1).encode().unwrap();
    wrong_profile[8] ^= 1;
    assert_eq!(
        ReactionDiffusionFieldState::decode(&wrong_profile),
        Err(ReactionDiffusionRefusal::WrongNumericProfile)
    );
    let mut wrong_magic = state(8, 8, 1).encode().unwrap();
    wrong_magic[0] ^= 1;
    assert_eq!(
        ReactionDiffusionFieldState::decode(&wrong_magic),
        Err(ReactionDiffusionRefusal::WrongMagic)
    );
    assert_eq!(
        ReactionDiffusionFieldState::decode(&wrong_magic[..20]),
        Err(ReactionDiffusionRefusal::WrongLength {
            expected: 64,
            actual: 20,
        })
    );
    let mut wrong_count = state(8, 8, 1).encode().unwrap();
    wrong_count[60..64].copy_from_slice(&63_u32.to_le_bytes());
    assert_eq!(
        ReactionDiffusionFieldState::decode(&wrong_count),
        Err(ReactionDiffusionRefusal::CellCountMismatch)
    );
    let mut out_of_range = state(8, 8, 1).encode().unwrap();
    out_of_range[64..68].copy_from_slice(&1_000_001_u32.to_le_bytes());
    assert_eq!(
        ReactionDiffusionFieldState::decode(&out_of_range),
        Err(ReactionDiffusionRefusal::ConcentrationOutOfRange)
    );
}

#[test]
fn identity_generation_and_work_are_admitted_before_evolution() {
    let initial = state(8, 8, 5);
    let canonical_request = request(0, 2, 128);
    assert_eq!(
        ReactionDiffusionEvolveRequest::decode(&canonical_request.encode()),
        Ok(canonical_request)
    );
    let mut noncanonical_request = canonical_request.encode();
    noncanonical_request[38] = 1;
    assert_eq!(
        ReactionDiffusionEvolveRequest::decode(&noncanonical_request),
        Err(ReactionDiffusionRefusal::WrongNumericProfile)
    );
    let wrong_id = ReactionDiffusionEvolveRequest {
        field_id: ReactionDiffusionFieldId(*b"field-other-0001"),
        ..request(0, 1, 64)
    };
    assert_eq!(
        initial.evolve_reference(wrong_id),
        Err(ReactionDiffusionRefusal::WrongFieldIdentity)
    );
    assert_eq!(
        initial.evolve_reference(request(1, 1, 64)),
        Err(ReactionDiffusionRefusal::StaleGeneration {
            expected: 0,
            actual: 1,
        })
    );
    assert_eq!(
        initial.evolve_reference(request(0, 2, 127)),
        Err(ReactionDiffusionRefusal::WorkLimitExceeded {
            required: 128,
            admitted: 127,
        })
    );
    assert_eq!(
        initial.evolve_reference(request(
            0,
            REACTION_DIFFUSION_MAXIMUM_GENERATIONS + 1,
            u32::MAX,
        )),
        Err(ReactionDiffusionRefusal::InvalidGenerationCount)
    );

    let at_end = ReactionDiffusionFieldState::from_cells(
        FIELD_ID,
        u64::MAX,
        initial.width,
        initial.height,
        initial.parameters,
        initial.cells().to_vec(),
    )
    .unwrap();
    assert_eq!(
        at_end.evolve_reference(ReactionDiffusionEvolveRequest {
            field_id: FIELD_ID,
            expected_generation: u64::MAX,
            generations: 1,
            admitted_cell_generations: 64,
        }),
        Err(ReactionDiffusionRefusal::GenerationOverflow)
    );
}

fn state(width: u16, height: u16, seed: u64) -> ReactionDiffusionFieldState {
    ReactionDiffusionFieldState::initialized(
        FIELD_ID,
        width,
        height,
        GrayScottParameters::REFERENCE,
        seed,
    )
    .unwrap()
}

fn request(
    expected_generation: u64,
    generations: u16,
    admitted_cell_generations: u32,
) -> ReactionDiffusionEvolveRequest {
    ReactionDiffusionEvolveRequest {
        field_id: FIELD_ID,
        expected_generation,
        generations,
        admitted_cell_generations,
    }
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_vec(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
