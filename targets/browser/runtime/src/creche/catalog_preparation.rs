//! One preparation-local installed catalog; no user Source receipts or Host truth cached.
use crate::installed_browser::{catalogs_for_presentation, PresentationProfile};
use conduit_plot::{CanonicalBackCatalog, ProfileCatalog, StartupCatalog};

#[derive(Default)]
pub(super) struct CatalogPreparation {
    current: Option<(
        PresentationProfile,
        StartupCatalog,
        ProfileCatalog,
        Option<CanonicalBackCatalog>,
    )>,
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
            self.current = Some((presentation, startup, profile, None));
        }
        let (_, startup, profile, _) = self.current.as_ref().expect("prepared catalog");
        Ok((startup, profile))
    }

    pub(super) fn get_with_backs(
        &mut self,
        presentation: PresentationProfile,
    ) -> Result<(&StartupCatalog, &ProfileCatalog, &CanonicalBackCatalog), String> {
        self.get(presentation)?;
        let (_, startup, profile, backs) = self.current.as_mut().expect("prepared catalog");
        if backs.is_none() {
            *backs = Some(crate::installed_browser::backs(startup, profile)?);
        }
        Ok((startup, profile, backs.as_ref().expect("prepared backs")))
    }
}
