//! Concrete hosted bootstrap at the Patchbay application edge.

use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use patchbay_model::PatchbayModel;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) fn fresh_model(
    composition: StdHostComposition,
    extend: impl FnOnce(&mut HostAdvertisement) -> Result<(), String>,
) -> Result<PatchbayModel, String> {
    let nonce = fresh_nonce();
    let host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from(format!("patchbay-native/{nonce}")),
            boot_id: BootId::from(format!("patchbay-boot/{nonce}")),
            offer_generation: OfferGeneration(1),
        },
        composition,
    );
    let mut advertisement = host.advertisement().clone();
    extend(&mut advertisement)?;
    Ok(PatchbayModel::from_advertisement(advertisement))
}

fn fresh_nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}-{sequence:x}")
}
