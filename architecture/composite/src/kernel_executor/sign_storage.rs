//! Additional finite Sign storage admitted by the containing Host before Play.

/// Extra local and remote lifecycle records for each prepared child kernel.
///
/// These bounds affect retained storage only. They do not grant effects, change
/// routing, discard history, or permit allocation during Play. Preparation
/// checks the complete item/byte envelopes before constructing the Sign log.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KernelCompositeSignStorage {
    pub additional_local_items: u16,
    pub additional_remote_items: u16,
}
