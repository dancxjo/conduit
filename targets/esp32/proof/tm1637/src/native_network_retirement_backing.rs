//! Preserve all actual Play custody until its final returned-owner slot exists.
use crate::native_play::{NativePlay, RetiredNetworkControl};
use alloc::boxed::Box;

#[inline(never)]
pub(crate) fn retain_restored(
    play: &mut NativePlay,
) -> Result<Box<RetiredNetworkControl>, &'static str> {
    let backing = crate::native_box_backing::reserve_box::<RetiredNetworkControl>()
        .map_err(|_| "network-retirement-capacity")?;
    // The unchanged actual scheduler/source/provider preflights remain the only
    // transfer gate. Refusal drops empty backing and leaves Play custody intact.
    let retired = play.take_retired()?;
    Ok(Box::write(backing, retired))
}
