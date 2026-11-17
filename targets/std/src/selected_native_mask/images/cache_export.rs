//! Compatibility entrance for the single shared operator cache codec.
use super::*;
impl NativeMaskImageData {
    pub fn write_cache(
        directory: &Path,
        deck: &crate::ambient_wallpaper::acquisition::PreparedDeck,
    ) -> Result<[u8; 32], String> {
        crate::ambient_wallpaper::cache::write_deck(directory, deck)
    }
}
