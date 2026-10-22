//! The same staged production initialization for each fixed-grant scenario.
use super::*;
pub(super) fn initialize(
    state: &mut State,
    applications: &[Option<Initial>; 4],
    transfer: &mut [u8],
) {
    assert!(matches!(
        exchange(
            state,
            &Command::PrepareInit {
                manifest: data::InitManifest::from_initial(applications),
            },
            transfer
        ),
        Reply::Prepared
    ));
    for (slot, initial) in applications.iter().enumerate() {
        match initial {
            None => {}
            Some(Initial::Patchbay { targets, .. }) => {
                for (target, value) in targets.iter().enumerate() {
                    if let Some(value) = value {
                        let component = on_caller(|| Command::PrepareTarget {
                            slot: slot as u8,
                            target: target as u8,
                            value: value.clone(),
                        });
                        assert!(matches!(
                            exchange(state, &component, transfer),
                            Reply::Prepared
                        ));
                        assert!(matches!(
                            exchange(
                                state,
                                &Command::FinishTarget {
                                    slot: slot as u8,
                                    target: target as u8,
                                },
                                transfer,
                            ),
                            Reply::Prepared
                        ));
                    }
                }
            }
            Some(Initial::Tutorial { evidence }) => {
                let component = on_caller(|| Command::PrepareTutorial {
                    slot: slot as u8,
                    evidence: evidence.clone(),
                });
                assert!(matches!(
                    exchange(state, &component, transfer),
                    Reply::Prepared
                ));
            }
            Some(Initial::Tour) => {
                assert!(matches!(
                    exchange(state, &Command::PrepareTour { slot: slot as u8 }, transfer),
                    Reply::Prepared
                ));
            }
        }
    }
    assert!(matches!(
        exchange(state, &Command::FinishInit, transfer),
        Reply::Initialized
    ));
}
