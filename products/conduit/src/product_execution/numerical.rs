//! Numerical realizations are offered before the ordinary local planning step.
use super::*;
impl ProductExecutionContext {
    pub(crate) fn local_std_for_plot(plot: &ExpandedCanonicalPlot) -> Result<Self, String> {
        let mut host = StdHost::new();
        host.prepare_numerical_realizations(plot)?;
        let advertisement = host.advertisement().clone();
        Self::new(
            vec![advertisement],
            vec![ProductRuntime::std(host)],
            vec![BaseImplementationId::from("conduit.base/local@1")],
            Vec::new(),
            Vec::new(),
        )
    }
}
