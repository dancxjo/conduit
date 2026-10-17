//! Value-semantic checked catalogues share immutable preparation snapshots.
//! Mutation copies a shared snapshot; no process-global catalogue is introduced.
use super::*;
#[cfg(not(target_has_atomic = "ptr"))]
use alloc::boxed::Box as Shared;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc as Shared;
use core::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CheckedPhysicalCatalogueData {
    pub dimensions: BTreeMap<String, DimensionDefinition>,
    pub quantities: BTreeMap<String, CheckedQuantityRole>,
    pub units: BTreeMap<String, UnitDefinition>,
    /// Accepted source spellings map to immutable canonical capsule spellings.
    pub aliases: BTreeMap<String, String>,
    pub unit_spans: BTreeMap<String, Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedPhysicalCatalogue {
    data: Shared<CheckedPhysicalCatalogueData>,
}
impl Default for CheckedPhysicalCatalogue {
    fn default() -> Self {
        Self {
            data: Shared::new(CheckedPhysicalCatalogueData::default()),
        }
    }
}
impl Deref for CheckedPhysicalCatalogue {
    type Target = CheckedPhysicalCatalogueData;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
impl DerefMut for CheckedPhysicalCatalogue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        #[cfg(target_has_atomic = "ptr")]
        {
            Shared::make_mut(&mut self.data)
        }
        // Keep original value semantics and Send/Sync on non-atomic targets.
        #[cfg(not(target_has_atomic = "ptr"))]
        {
            &mut self.data
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_clones_share_storage_and_mutation_keeps_the_original_snapshot() {
        let mut original = CheckedPhysicalCatalogue::default();
        original
            .aliases
            .insert("source-unit".into(), "capsule/original".into());
        let mut next = original.clone();
        assert_eq!(original, next);
        #[cfg(target_has_atomic = "ptr")]
        assert!(Shared::ptr_eq(&original.data, &next.data));
        next.aliases
            .insert("source-unit".into(), "capsule/next".into());
        assert_eq!(original.aliases["source-unit"], "capsule/original");
        assert_eq!(next.aliases["source-unit"], "capsule/next");
        assert_ne!(original, next);
        #[cfg(target_has_atomic = "ptr")]
        assert!(!Shared::ptr_eq(&original.data, &next.data));
    }
    #[test]
    fn checked_catalogue_preserves_send_sync_and_value_equality() {
        fn requires_send_sync<T: Send + Sync>() {}
        requires_send_sync::<CheckedPhysicalCatalogue>();
        let mut a = CheckedPhysicalCatalogue::default();
        let mut b = CheckedPhysicalCatalogue::default();
        a.aliases.insert("unit".into(), "capsule".into());
        b.aliases.insert("unit".into(), "capsule".into());
        assert_eq!(a, b);
    }
    #[test]
    #[ignore = "explicit characterization; does not claim whole-Planner or guest latency"]
    fn characterize_builtin_catalogue_clone_against_owned_reference() {
        let source = crate::parse_syntax_document(conduit_core::BUILTIN_PHYSICAL_SOURCE);
        let quantities = source
            .types
            .iter()
            .map(|declaration| {
                let crate::TypeDefinitionSyntax::Quantity(value) = &declaration.definition else {
                    panic!("builtin physical source contains a nonquantity Type");
                };
                (declaration.name.clone(), value.clone())
            })
            .collect::<Vec<_>>();
        let checked = check_physical_declarations(
            &source.dimensions,
            &source.prefixes,
            &quantities,
            &source.units,
        )
        .unwrap();
        assert!(!checked.units.is_empty());
        let profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        let storage = if cfg!(target_has_atomic = "ptr") {
            "shared-immutable"
        } else {
            "owned-nonatomic"
        };
        for trial in 0..8 {
            let mut shared_ns = 0;
            let mut owned_ns = 0;
            for shared in if trial % 2 == 0 {
                [true, false]
            } else {
                [false, true]
            } {
                let start = std::time::Instant::now();
                for _ in 0..64 {
                    if shared {
                        std::hint::black_box(checked.clone());
                    } else {
                        std::hint::black_box(checked.data.as_ref().clone());
                    }
                }
                let elapsed = start.elapsed().as_nanos();
                if shared {
                    shared_ns = elapsed;
                } else {
                    owned_ns = elapsed;
                }
            }
            std::println!("{{\"scope\":\"builtin-physical-catalogue-clone-and-drop\",\"trial\":{trial},\"iterations\":64,\"units\":{},\"quantities\":{},\"aliases\":{},\"shared_ns\":{shared_ns},\"owned_reference_ns\":{owned_ns},\"profile\":\"{profile}\",\"storage\":\"{storage}\",\"clock\":\"std::time::Instant\"}}",
                checked.units.len(), checked.quantities.len(), checked.aliases.len());
        }
    }
}
