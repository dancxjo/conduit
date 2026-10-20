//! Finite immutable Near strategy reuse, separate from Host admission and Play.
use conduit_core::{
    BootId, CapabilityOffer, CheckedPlotId, ExpandedPlotId, HostAdvertisement, HostId,
    HostProfileId, SourceDocumentId,
};
use conduit_plot::{CheckedGear, ExpandedCanonicalPlot};
use std::sync::Arc;

const MAX_FRAGMENTS: usize = 4;

#[derive(Clone, PartialEq, Eq)]
struct Context {
    protocol: u16,
    host: HostId,
    boot: BootId,
    profile: HostProfileId,
}
impl From<&HostAdvertisement> for Context {
    fn from(host: &HostAdvertisement) -> Self {
        Self {
            protocol: host.protocol_version,
            host: host.host_id.clone(),
            boot: host.boot_id.clone(),
            profile: host.profile.clone(),
        }
    }
}
struct Fragment {
    source: SourceDocumentId,
    checked: CheckedPlotId,
    expanded: ExpandedPlotId,
    gear: CheckedGear,
    context: Context,
    offers: Arc<[CapabilityOffer]>,
}
/// Four FIFO fragments per owning Host; input Source/Type structures are
/// caller-admitted finite preparation inputs. Retains no scratch or live Back.
/// The installed Back/artifact implementation is immutable within this binary.
/// Complete exact Gear equality fences configuration, Type, laws and precision;
/// source identities and Host/Boot/profile compatibility further restrict reuse.
#[derive(Default)]
pub(crate) struct NumericalPreparationCache {
    entries: Vec<Fragment>,
}
impl NumericalPreparationCache {
    pub(crate) fn near(
        &mut self,
        plot: &ExpandedCanonicalPlot,
        gear: &CheckedGear,
        host: &HostAdvertisement,
    ) -> Result<Arc<[CapabilityOffer]>, String> {
        let context = Context::from(host);
        if let Some(entry) = self.entries.iter().find(|entry| {
            entry.source == plot.source_document_id
                && entry.checked == plot.checked_plot_id
                && entry.expanded == plot.expanded_plot_id
                && entry.gear == *gear
                && entry.context == context
        }) {
            return Ok(entry.offers.clone());
        }
        // No hash-only key, caller cache token, or cached refusal is trusted.
        // The pure installed factory owns current artifact/version identities.
        let offers: Arc<[CapabilityOffer]> =
            crate::installed_std::near_offers(&gear.configuration)?.into();
        if self.entries.len() == MAX_FRAGMENTS {
            self.entries.remove(0);
        }
        self.entries.push(Fragment {
            source: plot.source_document_id.clone(),
            checked: plot.checked_plot_id.clone(),
            expanded: plot.expanded_plot_id.clone(),
            gear: gear.clone(),
            context,
            offers: offers.clone(),
        });
        Ok(offers)
    }
}

#[cfg(test)]
mod tests;
