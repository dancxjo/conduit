//! Finite outbox storage transition. This transfers only existing journal
//! custody; it does not establish retirement, a Body proposal, or a next Play.
use conduit_environment_runtime::display_evidence_journal::{Erased, Journal, Storage};
pub(crate) struct Cold<S> {
    pub(crate) region: Erased<S>,
    pub(crate) signed: bool,
    pub(crate) basis: &'static [u8],
    pub(crate) scope: Option<[u8; 96]>,
    pub(crate) capacity: u32,
    pub(crate) maximum_requested: u32,
}
/// Verify actual exclusive erased backing and return an UNBOUND storage slot.
/// Failure returns the same physical owner; this neither binds a scope nor
/// establishes native retirement, Body admission, signing or a next Play.
#[allow(clippy::result_large_err)]
pub(crate) fn prepare_cold<S: Storage>(
    storage: S,
    signed: bool,
    basis: &'static [u8],
) -> Result<
    Cold<S>,
    (
        S,
        conduit_environment_runtime::display_evidence_journal::Refusal,
    ),
> {
    let capacity = storage.capacity();
    Erased::prepare_retained(storage).map(|region| Cold {
        region,
        signed,
        basis,
        scope: None,
        capacity,
        maximum_requested: 0,
    })
}
pub(crate) struct Active<S> {
    pub(crate) journal: Journal<'static, S>,
    pub(crate) scope: [u8; 96],
    pub(crate) forms: [u8; 32],
    pub(crate) last_sequence: u64,
    pub(crate) failed: bool,
    pub(crate) recovered: bool,
    pub(crate) signed: bool,
    pub(crate) maximum_records: u32,
}
pub(crate) enum State<S> {
    Cold(Cold<S>),
    Active(Active<S>),
    #[cfg(all(feature = "local-appliance", target_os = "none"))]
    Pending(super::custody_journal::Custody<S>),
    #[cfg(all(feature = "local-appliance", target_os = "none"))]
    FrozenRegion(S),
    Failed,
}

/// Call only after Root has checked actual returned sampler custody and its
/// fresh installed proposal. Refusal mutates neither journal nor erased owner.
/// The old writer is frozen atomically before the new bound owner can escape.
pub(crate) fn rotate<S: Storage>(
    primary: &mut State<S>,
    epochs: &mut [State<S>; 5],
    old_scope: &[u8; 96],
    fresh_scope: [u8; 96],
    maximum: u32,
) -> Result<(), &'static str> {
    if maximum == 0
        || maximum > 960
        || old_scope[..64] != fresh_scope[..64]
        || old_scope[64..] == fresh_scope[64..]
    {
        return Err("observed-history-next-scope");
    }
    let State::Active(old) = primary else {
        return Err("observed-history-not-active");
    };
    if old.recovered || !old.signed || &old.scope != old_scope {
        return Err("observed-history-retired-scope");
    }
    let next = epochs
        .iter()
        .position(|state| {
            matches!(state,
                State::Cold(cold) if cold.signed && cold.scope.is_none()
                    && cold.capacity.saturating_sub(536) / 104 >= maximum
            )
        })
        .ok_or("observed-history-no-next-epoch")?;
    // No fallible work follows the complete owner/scope/capacity preflight.
    old.recovered = true;
    core::mem::swap(primary, &mut epochs[next]);
    let State::Cold(cold) = primary else {
        unreachable!("preflighted erased owner")
    };
    cold.scope = Some(fresh_scope);
    cold.maximum_requested = maximum;
    Ok(())
}

/// Fence the actual producer Play before projection failures, signing, or flash
/// may affect current custody. The caller obtains this digest from its checked
/// exact-generation Face lease, never from incoming publication data.
pub(crate) fn check_writer<S>(state: &State<S>, play: &[u8; 32]) -> Result<(), &'static str> {
    let scope = match state {
        State::Cold(cold) => cold.scope.as_ref().ok_or("observed-history-scope")?,
        State::Active(active) if !active.recovered => &active.scope,
        _ => return Err("observed-history-read-only"),
    };
    if scope[64..] != *play {
        return Err("observed-history-writer-play");
    }
    Ok(())
}

#[cfg(test)]
mod cold_tests {
    use super::*;
    use conduit_environment_runtime::display_evidence_journal::Refusal;
    struct Media {
        bytes: [u8; 112],
        fail: bool,
        identity: u32,
    }
    impl Storage for Media {
        fn capacity(&self) -> u32 {
            112
        }
        fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
            if self.fail {
                return Err(Refusal::Media);
            }
            out.copy_from_slice(&self.bytes[at as usize..at as usize + out.len()]);
            Ok(())
        }
        fn program(&mut self, _: u32, _: &[u8]) -> Result<(), Refusal> {
            panic!("Cold conversion cannot program");
        }
    }
    #[test]
    fn checked_erased_cold_is_unbound_and_preserves_exact_basis_without_programming() {
        let cold = prepare_cold(
            Media {
                bytes: [255; 112],
                fail: false,
                identity: 7,
            },
            true,
            b"checked canonical basis",
        )
        .ok()
        .unwrap();
        assert!(cold.signed);
        assert_eq!(cold.capacity, 112);
        assert!(cold.scope.is_none());
        assert_eq!(cold.maximum_requested, 0);
        assert_eq!(cold.basis, b"checked canonical basis");
        let state = State::Cold(cold);
        assert_eq!(
            check_writer(&state, &[0; 32]),
            Err("observed-history-scope")
        );
    }
    #[test]
    fn occupied_and_media_refusal_return_actual_owner_intact() {
        let mut bytes = [255; 112];
        bytes[100] = 0;
        let (storage, error) = prepare_cold(
            Media {
                bytes,
                fail: false,
                identity: 19,
            },
            true,
            b"basis",
        )
        .err()
        .unwrap();
        assert_eq!(error, Refusal::Corrupt);
        assert_eq!(storage.identity, 19);
        assert_eq!(storage.bytes[100], 0);
        let (storage, error) = prepare_cold(
            Media {
                bytes: [255; 112],
                fail: true,
                identity: 20,
            },
            true,
            b"basis",
        )
        .err()
        .unwrap();
        assert_eq!(error, Refusal::Media);
        assert_eq!(storage.identity, 20);
    }
}
