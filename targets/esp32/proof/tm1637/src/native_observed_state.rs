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
