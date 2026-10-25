//! Only the checked facts needed after node/configuration preparation.
use alloc::borrow::Cow;
#[cfg(feature = "indexed-source")]
use alloc::vec::Vec;
#[cfg(feature = "indexed-source")]
use conduit_core::GearId;
use conduit_core::{CheckedFront, ResourcePortContract};
use conduit_plot::CheckedGear;

#[cfg(feature = "indexed-source")]
pub(crate) struct ConnectionFacts {
    pub gear_id: GearId,
    pub front: CheckedFront,
    pub resource_ports: Vec<ResourcePortContract>,
}
pub(crate) enum ConnectionGear<'a> {
    Checked(&'a CheckedGear),
    #[cfg(feature = "indexed-source")]
    Prepared(&'a ConnectionFacts),
}
impl ConnectionGear<'_> {
    pub fn front(&self) -> Cow<'_, CheckedFront> {
        match self {
            Self::Checked(gear) => Cow::Owned(gear.checked_front()),
            #[cfg(feature = "indexed-source")]
            Self::Prepared(gear) => Cow::Borrowed(&gear.front),
        }
    }
    pub fn resource_ports(&self) -> &[ResourcePortContract] {
        match self {
            Self::Checked(gear) => &gear.resource_ports,
            #[cfg(feature = "indexed-source")]
            Self::Prepared(gear) => &gear.resource_ports,
        }
    }
}
