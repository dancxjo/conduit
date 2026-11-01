//! One authoritative Boot-local Store backing. Custody moves only its unique
//! pointer, never another Store-sized allocation. Image initialization resets
//! the once-cell every Boot; this is not DMA or retained/deep-sleep state.
use super::*;
#[cfg_attr(
    all(target_os = "none", feature = "local-appliance"),
    esp_hal::ram(unstable(rtc_slow))
)]
static BACKING: static_cell::StaticCell<Store> = static_cell::StaticCell::new();
pub(super) fn initialize(store: Store) -> Result<&'static mut Store, &'static str> {
    BACKING.try_init(store).ok_or("restart-store-already-owned")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_custody_preserves_once_only_initialization() {
        static CELL: static_cell::StaticCell<u32> = static_cell::StaticCell::new();
        let first = CELL.try_init(7).unwrap();
        assert!(CELL.try_init(9).is_none());
        assert_eq!(*first, 7);
        assert_eq!(
            core::mem::size_of::<StoreLease>(),
            core::mem::size_of::<usize>()
        );
        assert!(core::mem::size_of::<Slot>() <= 2 * core::mem::size_of::<usize>());
    }
}
