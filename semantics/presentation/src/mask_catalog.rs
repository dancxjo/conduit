//! Canonical value aliases used by ordinary Plots serving as Masks.
//!
//! This installs no Mask grammar and no second graph catalog. It only gives
//! ordinary Plot fores stable authored names for the three Face-role
//! values whose exact semantic identities are owned by this crate.

use conduit_core::kind_id;
use conduit_plot::StartupCatalog;

use crate::{FACE_INTERACTION_VALUE_KIND, PRESENTATION_VALUE_KIND, SHOW_VALUE_KIND};

pub fn install_mask_plot_value_aliases(
    startup: &mut StartupCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert_value_kind_alias("Presentation", kind_id(PRESENTATION_VALUE_KIND))?;
    startup.insert_value_kind_alias("FaceInteraction", kind_id(FACE_INTERACTION_VALUE_KIND))?;
    startup.insert_value_kind_alias("Show", kind_id(SHOW_VALUE_KIND))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_role_names_resolve_to_canonical_semantic_values() {
        let mut startup = StartupCatalog::new();
        install_mask_plot_value_aliases(&mut startup).unwrap();
        let source = conduit_plot::parse_syntax_document(
            "plot role (\n    >> face: Presentation\n    interaction: FaceInteraction...| >>\n    show: Show >>\n) {\n}\n",
        );
        let checked = conduit_plot::check_syntax_document(&source, &startup).unwrap();
        let front = &checked.plots[0].runtime_front;
        assert_eq!(
            front.inputs()[0].value_kind.as_str(),
            PRESENTATION_VALUE_KIND
        );
        assert_eq!(
            front.outputs()[0].value_kind.as_str(),
            FACE_INTERACTION_VALUE_KIND
        );
        assert_eq!(front.outputs()[1].value_kind.as_str(), SHOW_VALUE_KIND);
    }
}
