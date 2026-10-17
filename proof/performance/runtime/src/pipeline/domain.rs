//! Representative domain measurements through installed production execution.
use super::*;
mod control;
mod numerics;
pub(super) mod observation;
use observation::Outputs;
pub(super) fn run(
    config: &Config,
    raw: &mut artifact::Raw,
    summaries: &mut Vec<Value>,
) -> Result<(), Box<dyn Error>> {
    control::run(config, raw, summaries)?;
    numerics::run(config, raw, summaries)
}
