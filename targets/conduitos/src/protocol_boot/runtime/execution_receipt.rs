//! Bound receipts never control retirement or replace an original refusal.
use super::*;

pub(super) fn before_stop(
    play: &NativePlay,
    allocation_before: (u64, u64),
    allocation_after: (u64, u64),
) -> Option<crate::sign_format::FixedText> {
    let mut stopped = crate::sign_format::FixedText::new();
    let mut stopped_bound = false;
    if let Some(identity) = play.session().realization().and_then(|r| r.play.as_ref()) {
        use core::fmt::Write;
        let mut receipt = crate::sign_format::FixedText::new();
        if writeln!(
            receipt,
            "CONDUIT_PROTOCOL_PLAY_ALLOCATIONS body={} plan={} play={} alloc={} realloc={}",
            identity.body_id.as_str(),
            identity.plan_id.as_str(),
            identity.active_play_id.as_str(),
            allocation_after.0.saturating_sub(allocation_before.0),
            allocation_after.1.saturating_sub(allocation_before.1),
        )
        .is_ok()
        {
            let _ = arch::append_boot_diagnostic(receipt.as_bytes());
        }
        stopped_bound = writeln!(
            stopped,
            "CONDUIT_PROTOCOL_PLAY_STOPPED body={} plan={} play={}",
            identity.body_id.as_str(),
            identity.plan_id.as_str(),
            identity.active_play_id.as_str(),
        )
        .is_ok();
    }
    stopped_bound.then_some(stopped)
}
