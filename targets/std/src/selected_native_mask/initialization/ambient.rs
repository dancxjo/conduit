//! Ambient worker composition through the native display constructor.
use super::*;

impl StdHost {
    /// Compose Ambient and the actual native output under one original IMAGE.
    ///
    /// # Safety
    /// The Ambient grant and deck factory must meet the same authorization
    /// and finite provider bounds as the screen-free Ambient constructor.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_selected_ambient_home_and_native_mask_image<F>(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &CheckedHostConfiguration,
        parameters: CheckedNativeMaskInitialization,
        output: &mut conduit_linux_display::Scanout<'_>,
        playback: Option<&crate::hosted_audio::HostedPlaybackSelection>,
        grant: conduit_core::AuthorityGrant,
        prepare_deck: F,
    ) -> Result<Self, String>
    where
        F: FnOnce() -> Result<crate::ambient_wallpaper::observation::WallpaperPreparation, String>,
    {
        Self::from_native_mask_backend(
            config,
            image,
            catalog,
            checked,
            parameters,
            output,
            false,
            None,
            None,
            None,
            playback,
            Some(AmbientWorkerStartup {
                grant,
                prepare_deck: Box::new(prepare_deck),
            }),
        )
    }
}
