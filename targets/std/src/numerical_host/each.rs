//! Publish Source-selected numeric and Each offers as one preparation batch.
use super::*;
impl StdHost {
    pub fn prepare_near_each_realization(
        &mut self,
        activation: &conduit_plot::ExpandedActivation,
        child: &ExpandedCanonicalPlot,
    ) -> Result<(), String> {
        let conduit_plot::ActivationSyntax::Each { maximum_items } = activation.mode else {
            return Err("numerical Fore supports an authored Each activation".into());
        };
        if activation.selected_checked_plot_id != child.checked_plot_id
            || activation.selected_plot != child.name
            || child.gears.len() != 1
            || child.gears[0].kind_id.as_str() != conduit_semantic_catalog::NUMERICAL_NEAR_KIND
            || !child.connections.is_empty()
            || !child.activations.is_empty()
            || !child.shared_pools.is_empty()
        {
            return Err("numerical Each requires its exact single Near Source child".into());
        }
        let offer = crate::flow_activation::near_each_offer(
            &activation.input_contract,
            &activation.output_contract,
            maximum_items,
        )?;
        self.prepare_numerical_batch(child, Some(offer))
    }
}
