//! Stateful admitted std-host boundary for fixed-point Lenia evolution.

use super::alife_operations::parameters;
use conduit_alife::{LeniaEngine, LeniaFieldView, LENIA_MAXIMUM_FIELD_BYTES, LENIA_Q16_ONE};
use conduit_core::{
    ConfigurationValue, HostOperationContractId, KindId, PlanFragment, PlannedGear,
};
use conduit_kernel::{Failure, FailureCode, NodeId};
use std::io::Write;

pub(super) struct AlifeHost {
    engines: Vec<Option<LeniaEngine>>,
    output: Vec<u8>,
}

pub(super) enum AlifeCompletion<'a> {
    Completed,
    Output(&'a [u8]),
    Failed(Failure),
}

impl AlifeHost {
    pub(super) fn prepare(fragment: &PlanFragment) -> Result<Self, String> {
        let mut engines = Vec::with_capacity(fragment.placements.len());
        for placement in &fragment.placements {
            if placement.implementation_id.as_str()
                == conduit_std_catalog::LENIA_STEP_IMPLEMENTATION
            {
                engines
                    .push(Some(LeniaEngine::new(parameters(placement)?).map_err(
                        |error| format!("prepare Lenia engine: {error:?}"),
                    )?));
            } else {
                engines.push(None);
            }
        }
        Ok(Self {
            engines,
            output: Vec::with_capacity(LENIA_MAXIMUM_FIELD_BYTES as usize),
        })
    }

    pub(super) fn initialize(&mut self, node: NodeId, input: &[u8]) -> Result<(), Failure> {
        self.engine(node)?.initialize(input).map_err(|_| Failure {
            code: FailureCode::InvalidInput,
            detail: 192,
        })
    }

    pub(super) fn step(&mut self, node: NodeId, tick: &[u8]) -> Result<&[u8], Failure> {
        super::contract::decode_tick(tick).map_err(|_| Failure {
            code: FailureCode::InvalidInput,
            detail: 193,
        })?;
        let index = usize::from(node.0);
        let engine = self
            .engines
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or(Failure {
                code: FailureCode::HostOperationDenied,
                detail: 194,
            })?;
        engine.step_into(&mut self.output).map_err(|_| Failure {
            code: FailureCode::HostOperationFailed,
            detail: 195,
        })?;
        Ok(&self.output)
    }

    pub(super) fn allocation_capacity(&self) -> usize {
        self.output.capacity()
            + self.engines.capacity()
            + self
                .engines
                .iter()
                .flatten()
                .map(LeniaEngine::allocation_capacity)
                .sum::<usize>()
    }

    pub(super) fn execute<'a, W: Write>(
        &'a mut self,
        contract: &HostOperationContractId,
        target: Option<&KindId>,
        node: NodeId,
        input: &[u8],
        fragment: &PlanFragment,
        output: &mut W,
    ) -> Option<AlifeCompletion<'a>> {
        if contract.as_str() == conduit_std_catalog::LENIA_INITIALIZE_HOST_OPERATION
            && target.is_some_and(|kind| kind.as_str() == conduit_alife::LENIA_STEP_KIND)
        {
            return Some(match self.initialize(node, input) {
                Ok(()) => AlifeCompletion::Completed,
                Err(failure) => AlifeCompletion::Failed(failure),
            });
        }
        if contract.as_str() == conduit_std_catalog::LENIA_STEP_HOST_OPERATION
            && target.is_some_and(|kind| kind.as_str() == conduit_alife::LENIA_STEP_KIND)
        {
            return Some(match self.step(node, input) {
                Ok(encoded) => AlifeCompletion::Output(encoded),
                Err(failure) => AlifeCompletion::Failed(failure),
            });
        }
        if contract.as_str() == conduit_core::PRESENT_HOST_OPERATION_CONTRACT
            && target.is_some_and(|kind| {
                kind.as_str() == conduit_std_catalog::SCALAR_FIELD_PRESENTATION_TARGET
            })
        {
            let Some(placement) = fragment.placements.get(usize::from(node.0)) else {
                return Some(AlifeCompletion::Failed(Failure {
                    code: FailureCode::HostOperationDenied,
                    detail: 205,
                }));
            };
            return Some(match present(placement, input, output) {
                Ok(()) => AlifeCompletion::Completed,
                Err(failure) => AlifeCompletion::Failed(failure),
            });
        }
        None
    }

    fn engine(&mut self, node: NodeId) -> Result<&mut LeniaEngine, Failure> {
        self.engines
            .get_mut(usize::from(node.0))
            .and_then(Option::as_mut)
            .ok_or(Failure {
                code: FailureCode::HostOperationDenied,
                detail: 196,
            })
    }
}

fn present<W: Write>(placement: &PlannedGear, input: &[u8], output: &mut W) -> Result<(), Failure> {
    let view = LeniaFieldView::decode(input).map_err(|_| Failure {
        code: FailureCode::InvalidInput,
        detail: 197,
    })?;
    let title = text_configuration(placement, conduit_alife::TITLE_KEY)?;
    let minimum = scalar_q16(placement, conduit_alife::MINIMUM_KEY)?;
    let maximum = scalar_q16(placement, conduit_alife::MAXIMUM_KEY)?;
    if minimum >= maximum {
        return Err(Failure {
            code: FailureCode::InvalidInput,
            detail: 198,
        });
    }
    writeln!(
        output,
        "SCALAR-FIELD title={title:?} generation={} width={} height={} profile={}",
        view.header.generation,
        view.header.width,
        view.header.height,
        conduit_alife::LENIA_NUMERIC_PROFILE,
    )
    .map_err(|_| io_failure())?;
    const COLUMNS: usize = 32;
    const ROWS: usize = 16;
    const RAMP: &[u8] = b" .:-=+*#%@";
    let width = usize::from(view.header.width);
    let height = usize::from(view.header.height);
    for display_y in 0..ROWS {
        let start_y = display_y * height / ROWS;
        let end_y = ((display_y + 1) * height / ROWS).max(start_y + 1);
        for display_x in 0..COLUMNS {
            let start_x = display_x * width / COLUMNS;
            let end_x = ((display_x + 1) * width / COLUMNS).max(start_x + 1);
            let mut total = 0_u64;
            let mut count = 0_u64;
            for y in start_y..end_y {
                for x in start_x..end_x {
                    total += u64::from(view.cell(y * width + x).map_err(|_| Failure {
                        code: FailureCode::InvalidInput,
                        detail: 199,
                    })?);
                    count += 1;
                }
            }
            let average = (total / count) as u32;
            let normalized = average.saturating_sub(minimum).min(maximum - minimum);
            let ramp_index = usize::try_from(
                u64::from(normalized) * (RAMP.len() as u64 - 1) / u64::from(maximum - minimum),
            )
            .map_err(|_| io_failure())?;
            output
                .write_all(&RAMP[ramp_index..=ramp_index])
                .map_err(|_| io_failure())?;
        }
        output.write_all(b"\n").map_err(|_| io_failure())?;
    }
    Ok(())
}

fn text_configuration<'a>(placement: &'a PlannedGear, key: &str) -> Result<&'a str, Failure> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            (actual, ConfigurationValue::Text(value)) if actual == key => Some(value.as_str()),
            _ => None,
        })
        .ok_or(Failure {
            code: FailureCode::InvalidInput,
            detail: 200,
        })
}

fn scalar_q16(placement: &PlannedGear, key: &str) -> Result<u32, Failure> {
    let value = placement
        .configuration
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            (actual, ConfigurationValue::I64(value)) if actual == key => Some(*value),
            _ => None,
        })
        .ok_or(Failure {
            code: FailureCode::InvalidInput,
            detail: 201,
        })?;
    if !(0..=conduit_core::Scalar::SCALE).contains(&value) {
        return Err(Failure {
            code: FailureCode::InvalidInput,
            detail: 202,
        });
    }
    u32::try_from(
        (i128::from(value) * i128::from(LENIA_Q16_ONE)
            + i128::from(conduit_core::Scalar::SCALE / 2))
            / i128::from(conduit_core::Scalar::SCALE),
    )
    .map_err(|_| Failure {
        code: FailureCode::InvalidInput,
        detail: 203,
    })
}

fn io_failure() -> Failure {
    Failure {
        code: FailureCode::HostOperationFailed,
        detail: 204,
    }
}
