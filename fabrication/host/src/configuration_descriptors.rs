use std::collections::BTreeMap;

use crate::{FabricationPackageSet, TargetDescriptor};

pub fn compatible_base_implementations(
    descriptor: &TargetDescriptor,
    packages: &FabricationPackageSet,
) -> Vec<(String, Vec<String>)> {
    let mut choices = BTreeMap::<String, Vec<String>>::new();
    for resolved in packages.offers_for_target(&descriptor.key()) {
        choices
            .entry(resolved.offer.base_kind)
            .or_default()
            .push(resolved.offer.implementation_id);
    }
    choices
        .into_iter()
        .map(|(kind, mut implementations)| {
            implementations.sort();
            implementations.dedup();
            (kind, implementations)
        })
        .collect()
}
