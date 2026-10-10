//! One preparation-local installed catalog; no Source receipts or Host truth cached.
use crate::installed_browser::{catalogs_for_presentation, PresentationProfile};
use conduit_plot::{ProfileCatalog, StartupCatalog};

#[derive(Default)]
pub(super) struct CatalogPreparation {
    current: Option<(PresentationProfile, StartupCatalog, ProfileCatalog)>,
}
impl CatalogPreparation {
    pub(super) fn get(
        &mut self,
        presentation: PresentationProfile,
    ) -> Result<(&StartupCatalog, &ProfileCatalog), String> {
        if self
            .current
            .as_ref()
            .is_none_or(|entry| entry.0 != presentation)
        {
            // Drop the previous profile before preparing a replacement. The
            // retained bound is one pair even for mixed presentation bundles.
            self.current = None;
            let (startup, profile) = catalogs_for_presentation(presentation)?;
            self.current = Some((presentation, startup, profile));
        }
        let (_, startup, profile) = self.current.as_ref().expect("prepared catalog");
        Ok((startup, profile))
    }
}
