//! Reviewed checked documents, not runtime-supplied configuration authority.
use conduit_face::{
    patchbay_component_mask_root, ComponentMaskLook, ComponentVisualSelection,
    PATCHBAY_COMPONENT_MASK_SOURCE, PATCHBAY_DEFAULT_MASK_ROOT, TODO_COMPONENT_MASK_SOURCE,
};

pub(super) fn contains(source: &str) -> bool {
    [TODO_COMPONENT_MASK_SOURCE, PATCHBAY_COMPONENT_MASK_SOURCE].contains(&source)
}

pub(super) fn for_root(root: &str) -> Option<&'static str> {
    if ComponentVisualSelection::from_todo_root(root).is_some() {
        return Some(TODO_COMPONENT_MASK_SOURCE);
    }
    if root == PATCHBAY_DEFAULT_MASK_ROOT
        || [
            ComponentMaskLook::Compact,
            ComponentMaskLook::Spacious,
            ComponentMaskLook::CompactEditorial,
            ComponentMaskLook::SpaciousPhosphor,
        ]
        .iter()
        .any(|look| patchbay_component_mask_root(*look) == root)
    {
        return Some(PATCHBAY_COMPONENT_MASK_SOURCE);
    }
    None
}

#[cfg(test)]
#[path = "configured_sources_tests.rs"]
mod tests;
