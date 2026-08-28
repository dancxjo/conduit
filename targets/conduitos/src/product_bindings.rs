//! Finite Presenter-local bindings for the ConduitOS product journey.

use patchbay_control::PatchbayAction;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductBinding {
    pub usage: u8,
    pub label: &'static str,
    pub action: PatchbayAction,
}

const PRODUCT_BINDINGS: [ProductBinding; 6] = [
    ProductBinding {
        usage: 60,
        label: "F3",
        action: PatchbayAction::Birth,
    },
    ProductBinding {
        usage: 61,
        label: "F4",
        action: PatchbayAction::Wake,
    },
    ProductBinding {
        usage: 62,
        label: "F5",
        action: PatchbayAction::Plan,
    },
    ProductBinding {
        usage: 63,
        label: "F6",
        action: PatchbayAction::Play,
    },
    ProductBinding {
        usage: 64,
        label: "F7",
        action: PatchbayAction::Lull,
    },
    ProductBinding {
        usage: 65,
        label: "F8",
        action: PatchbayAction::Stop,
    },
];

#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
pub(crate) fn binding_for_usage(usage: u8) -> Option<ProductBinding> {
    PRODUCT_BINDINGS
        .iter()
        .copied()
        .find(|binding| binding.usage == usage)
}

pub(crate) fn binding_for_intent(intent: &str) -> Option<ProductBinding> {
    PRODUCT_BINDINGS
        .iter()
        .copied()
        .find(|binding| binding.action.presentation_intent() == intent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_bindings_are_finite_unique_and_bidirectional() {
        assert_eq!(PRODUCT_BINDINGS.len(), 6);
        for (index, binding) in PRODUCT_BINDINGS.iter().enumerate() {
            assert!(
                PRODUCT_BINDINGS[..index]
                    .iter()
                    .all(|prior| prior.usage != binding.usage)
            );
            assert!(
                PRODUCT_BINDINGS[..index]
                    .iter()
                    .all(|prior| prior.label != binding.label)
            );
            assert!(
                PRODUCT_BINDINGS[..index]
                    .iter()
                    .all(|prior| prior.action != binding.action)
            );
            assert_eq!(binding_for_usage(binding.usage), Some(*binding));
            assert_eq!(
                binding_for_intent(binding.action.presentation_intent()),
                Some(*binding)
            );
        }
    }

    #[test]
    fn unrelated_input_and_semantics_have_no_product_binding() {
        assert_eq!(binding_for_usage(40), None);
        assert_eq!(binding_for_usage(u8::MAX), None);
        assert_eq!(
            binding_for_intent(PatchbayAction::OpenBack.presentation_intent()),
            None
        );
        assert_eq!(binding_for_intent("conduit.intent/unknown@1"), None);
    }
}
