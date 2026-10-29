//! Shared canonical ordering for Root's descriptive provider projections. This
//! does not validate an offer, issue authority, or move any retained provider.
use conduit_core::HostAdvertisement;
#[inline(never)]
pub(crate) fn sort(advertisement: &mut HostAdvertisement) {
    advertisement
        .bases
        .sort_by(|a, b| a.base_id.cmp(&b.base_id));
    advertisement
        .resources
        .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
}
