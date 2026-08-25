use super::*;

#[test]
fn fixed_profile_is_repeatable_and_does_not_grow_after_prepare() {
    let seed = crate::orbium_seed(32, 32, 1).unwrap().encode().unwrap();
    let mut first = LeniaEngine::new(LeniaParameters::ORBIUM).unwrap();
    let mut second = LeniaEngine::new(LeniaParameters::ORBIUM).unwrap();
    let first_capacity = first.allocation_capacity();
    let second_capacity = second.allocation_capacity();
    let mut first_output = Vec::with_capacity(LENIA_MAXIMUM_FIELD_BYTES as usize);
    let mut second_output = Vec::with_capacity(LENIA_MAXIMUM_FIELD_BYTES as usize);
    first.initialize(&seed).unwrap();
    second.initialize(&seed).unwrap();
    for _ in 0..4 {
        first.step_into(&mut first_output).unwrap();
        second.step_into(&mut second_output).unwrap();
    }
    assert_eq!(first_output, second_output);
    assert_eq!(first.allocation_capacity(), first_capacity);
    assert_eq!(second.allocation_capacity(), second_capacity);
    let evolved = LeniaFieldState::decode(&first_output).unwrap();
    assert_eq!(evolved.generation, 4);
    assert_eq!(
        evolved.semantic_digest().unwrap(),
        [
            151, 17, 74, 246, 124, 134, 213, 100, 238, 39, 245, 209, 222, 186, 75, 250, 83, 189,
            41, 101, 79, 154, 80, 210, 46, 16, 60, 197, 213, 9, 147, 74,
        ]
    );
}

#[test]
fn malformed_fields_and_invalid_parameters_are_distinct_refusals() {
    assert_eq!(
        LeniaFieldState::from_cells(LeniaFieldId([0; 16]), 0, 31, 32, vec![0; 31 * 32]),
        Err(LeniaRefusal::InvalidDimensions)
    );
    assert_eq!(
        LeniaFieldState::from_cells(
            LeniaFieldId([0; 16]),
            0,
            32,
            32,
            vec![LENIA_Q16_ONE + 1; 32 * 32],
        ),
        Err(LeniaRefusal::CellOutOfRange)
    );
    let mut malformed = crate::orbium_seed(32, 32, 1).unwrap().encode().unwrap();
    malformed[0] = 0;
    assert_eq!(
        LeniaFieldState::decode(&malformed),
        Err(LeniaRefusal::WrongMagic)
    );
    let mut invalid = LeniaParameters::ORBIUM;
    invalid.kernel_radius = 0;
    assert_eq!(
        LeniaEngine::new(invalid).err(),
        Some(LeniaRefusal::InvalidParameters)
    );
}
