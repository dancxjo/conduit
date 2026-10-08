//! Reusable public product entrances shared by packaged Conduit applications.

mod hosted_two_std;
mod plot_source;
mod product_execution;
pub mod todo_action_bridge;

use std::{io::Write, path::Path};

use conduit_core::{HostAdvertisement, LineOffer, Observation, Plan};

pub use hosted_two_std::{execute_hosted_two_std_plot, HostedTwoHostExecution};

/// Exact result of running authored source through the installed hosted Host.
pub struct HostedPlotExecution {
    pub authored_source: String,
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_plot_id: conduit_core::CheckedPlotId,
    pub expanded_plot_id: conduit_core::ExpandedPlotId,
    pub plan: Plan,
    pub observations: Vec<Observation>,
    pub advertisements: Vec<HostAdvertisement>,
    pub line_offers: Vec<LineOffer>,
}

/// Check, expand, plan, admit, and execute one authored plot on the real local std Host.
///
/// This is the library-shaped equivalent of `conduit run`, retained so a native
/// application presenter does not create a second execution kernel or shell out
/// to an untyped compatibility entrance.
pub fn execute_hosted_plot(
    source: &str,
    output: &mut impl Write,
) -> Result<HostedPlotExecution, String> {
    execute_source(plot_source::parse(source)?, output, None, false)
}

/// File-based plot of [`execute_hosted_plot`], used by the installed CLI entrance.
pub fn execute_hosted_plot_file(
    path: &Path,
    output: &mut impl Write,
) -> Result<HostedPlotExecution, String> {
    execute_source(plot_source::load(path)?, output, None, false)
}

/// Execute with an explicit admitted Stop channel for standing Plots.
pub fn execute_hosted_plot_controlled(
    source: &str,
    output: &mut impl Write,
    control: &conduit_std_host::RunControl,
) -> Result<HostedPlotExecution, String> {
    execute_source(plot_source::parse(source)?, output, Some(control), false)
}

/// Recursively realize reviewed Backs and execute with an explicit Stop channel.
pub fn execute_hosted_plot_recursive_controlled(
    source: &str,
    output: &mut impl Write,
    control: &conduit_std_host::RunControl,
) -> Result<HostedPlotExecution, String> {
    execute_source(plot_source::parse(source)?, output, Some(control), true)
}

fn execute_source(
    source: plot_source::CanonicalSource,
    output: &mut impl Write,
    control: Option<&conduit_std_host::RunControl>,
    recursive: bool,
) -> Result<HostedPlotExecution, String> {
    let plot = if recursive {
        source.expand_entry_recursive()?
    } else {
        source.expand_entry()?
    };
    let mut context = product_execution::ProductExecutionContext::local_std()?;
    let plan = context.plan(&plot, None)?;
    let execution = match control {
        Some(control) => context.execute_attached(plan, output, control)?,
        None => context.execute(plan, output)?,
    };
    Ok(HostedPlotExecution {
        authored_source: source.source,
        source_document_id: plot.source_document_id,
        checked_plot_id: plot.checked_plot_id,
        expanded_plot_id: plot.expanded_plot_id,
        plan: execution.plan,
        observations: execution.observations,
        advertisements: execution.advertisements,
        line_offers: execution.line_offers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_entrance_runs_the_same_real_hosted_plot_pipeline() {
        let mut output = Vec::new();
        let execution = execute_hosted_plot(
            "plot hosted-library {\n  words: text/literal(\"hello\")\n  change: text/upper\n  result: presentation/text\n  words >> change >> result\n}.\n",
            &mut output,
        )
        .unwrap();
        assert!(String::from_utf8(output).unwrap().contains("\nHELLO\n"));
        assert!(!execution.plan.plan_id.as_str().is_empty());
        assert!(execution
            .observations
            .iter()
            .any(|observation| observation.active_play_id.is_some()));
    }

    #[test]
    fn two_host_entrance_retains_fragments_plays_line_and_delivery() {
        let mut output = Vec::new();
        let execution = execute_hosted_two_std_plot(
            "plot hello-across {\n  message: text/literal(\"hello across one Cord\")\n  show: presentation/text\n  message >> show\n}\n",
            &mut output,
        )
        .unwrap();
        assert_eq!(execution.result, "hello across one Cord");
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "hello across one Cord\n"
        );
        assert_ne!(execution.source_fragment_id, execution.sink_fragment_id);
        assert_ne!(
            execution.source_active_play_id,
            execution.sink_active_play_id
        );
        assert_eq!(execution.transferred_values, 1);
        assert!(!execution.line_id.as_str().is_empty());
    }
}
