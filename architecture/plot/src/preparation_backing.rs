//! Conservative reachable preparation storage; never a factory-peak claim.
//! Shared allocations are charged in full for every alias. This does not lend
//! their backing to another owner. Inline values and allocator metadata excluded.
use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};
use core::mem::{align_of, size_of};
pub(crate) trait Backing {
    fn backing(&self) -> Option<usize>;
}
pub(crate) fn sum(values: impl IntoIterator<Item = Option<usize>>) -> Option<usize> {
    values
        .into_iter()
        .try_fold(0usize, |a, b| a.checked_add(b?))
}
impl Backing for String {
    fn backing(&self) -> Option<usize> {
        Some(self.capacity())
    }
}
impl<T: Backing> Backing for Vec<T> {
    fn backing(&self) -> Option<usize> {
        sum(
            core::iter::once(self.capacity().checked_mul(size_of::<T>()))
                .chain(self.iter().map(Backing::backing)),
        )
    }
}
impl<T: Backing> Backing for Option<T> {
    fn backing(&self) -> Option<usize> {
        self.as_ref().map_or(Some(0), Backing::backing)
    }
}
impl<T: Backing> Backing for Box<T> {
    fn backing(&self) -> Option<usize> {
        size_of::<T>().checked_add((**self).backing()?)
    }
}
impl<A: Backing, B: Backing> Backing for (A, B) {
    fn backing(&self) -> Option<usize> {
        sum([self.0.backing(), self.1.backing()])
    }
}
impl<K: Backing, V: Backing> Backing for BTreeMap<K, V> {
    fn backing(&self) -> Option<usize> {
        // Pinned Rust 1.98.1 alloc collections/btree/node.rs uses B=6: at most 11 key/value slots and 12
        // edges per node. One node per live entry plus a possible empty root
        // bounds occupancy without inspecting private allocator representation.
        // Padding is deliberately overcounted. Requalify if alloc changes B.
        let entry = sum([
            Some(size_of::<K>()),
            Some(size_of::<V>()),
            Some(align_of::<K>()),
            Some(align_of::<V>()),
        ])?;
        let node = entry
            .checked_mul(11)?
            .checked_add(16usize.checked_mul(size_of::<usize>())?)?;
        let nodes = self.len().checked_add(1)?.checked_mul(node)?;
        sum(core::iter::once(Some(nodes))
            .chain(self.iter().map(|(k, v)| sum([k.backing(), v.backing()]))))
    }
}
macro_rules! identity { ($($t:ty),* $(,)?) => { $(impl Backing for $t {
    fn backing(&self) -> Option<usize> { Some(self.0.capacity()) }
})* }; }
identity!(
    conduit_core::KindId,
    conduit_core::KindIdentity,
    conduit_core::PortId,
    conduit_core::SourceDocumentId,
    conduit_core::CheckedPlotId
);
impl Backing for conduit_core::PortDescriptor {
    fn backing(&self) -> Option<usize> {
        sum([
            self.port_id.backing(),
            self.value_kind.backing(),
            self.abnormal_kind.backing(),
        ])
    }
}
impl Backing for conduit_core::FrontStartupParameter {
    fn backing(&self) -> Option<usize> {
        sum([self.name.backing(), self.value_type.backing()])
    }
}
impl Backing for conduit_core::StructuredInfoType {
    fn backing(&self) -> Option<usize> {
        self.retained_backing_bytes()
    }
}
impl Backing for conduit_core::CheckedValueContract {
    fn backing(&self) -> Option<usize> {
        self.retained_backing_bytes()
    }
}
impl Backing for conduit_core::CheckedFront {
    fn backing(&self) -> Option<usize> {
        self.retained_backing_bytes()
    }
}
impl Backing for conduit_core::Kind {
    fn backing(&self) -> Option<usize> {
        self.retained_backing_bytes()
    }
}
impl Backing for conduit_core::PlotBack {
    fn backing(&self) -> Option<usize> {
        sum([
            self.invocation_path.backing(),
            self.kind_id.backing(),
            self.kind_contract_revision.backing(),
            self.source_document_id.backing(),
            self.checked_plot_id.backing(),
        ])
    }
}
impl Backing for conduit_core::KindConfigurationField {
    fn backing(&self) -> Option<usize> {
        self.retained_backing_bytes()
    }
}

#[cfg(test)]
mod tests;
