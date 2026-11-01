use conduit_core::*;
#[test]
fn borrowed_sign_budget_is_the_exact_existing_storage_law() {
    let signs = [
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlacementPrepared("placement\0é".into()),
        ExpectedSign::PlacementTerminal("terminal".into()),
        ExpectedSign::ConnectionTerminal("cord".into()),
        ExpectedSign::PlanTerminal,
    ];
    let borrowed: Vec<_> = signs.iter().map(Into::into).collect();
    for n in 0..=signs.len() {
        assert_eq!(
            mandatory_sign_storage_requirement(&signs[..n]),
            mandatory_sign_storage_requirement_ref(ExpectedSignRefs::Borrowed(&borrowed[..n]))
        );
    }
    for n in [usize::from(u16::MAX), usize::from(u16::MAX) + 1] {
        let owned = vec![ExpectedSign::PlanTerminal; n];
        let borrowed = vec![ExpectedSignRef::PlanTerminal; n];
        assert_eq!(
            mandatory_sign_storage_requirement(&owned),
            mandatory_sign_storage_requirement_ref(ExpectedSignRefs::Borrowed(&borrowed))
        );
        assert_eq!(
            mandatory_sign_storage_requirement(&owned).is_some(),
            n <= usize::from(u16::MAX)
        );
    }
}
