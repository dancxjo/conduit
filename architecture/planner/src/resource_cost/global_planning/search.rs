//! Finite assignment traversal and ordinary Plan validation.
use super::*;
use core::cmp::Ordering;

pub(super) struct Search<'a, 'b, 'c> {
    pub(super) plot: &'b CheckedPlot,
    pub(super) inputs: CostPlanningInputs<'a, 'c>,
    pub(super) grids: &'b [Vec<Candidate<'a>>],
    pub(super) requirements: &'b BTreeMap<conduit_core::GearId, crate::HardRealizationRequirements>,
    pub(super) maximum_states: u32,
    pub(super) states: u32,
    pub(super) complete: u32,
    pub(super) refused: u32,
    pub(super) disposition: GlobalPlacementDisposition,
    pub(super) best: Option<(Plan, GlobalPlacementScore)>,
    pub(super) work: &'b mut ExactNumericWork,
    pub(super) objective: GlobalPlacementObjective,
    pub(super) transfer: Option<GlobalTransferInputs<'a>>,
    pub(super) order: Option<&'b [usize]>,
}
impl Search<'_, '_, '_> {
    pub(super) fn visit(&mut self, depth: usize, chosen: &mut Vec<usize>) {
        if self.disposition != GlobalPlacementDisposition::Exhaustive {
            return;
        }
        if self.states == self.maximum_states {
            self.disposition = GlobalPlacementDisposition::StateLimit;
            return;
        }
        self.states += 1;
        if depth == self.grids.len() {
            self.finish(chosen);
            return;
        }
        for index in 0..self.grids[depth].len() {
            chosen.push(index);
            self.visit(depth + 1, chosen);
            chosen.pop();
            if self.disposition != GlobalPlacementDisposition::Exhaustive {
                break;
            }
        }
    }
    fn finish(&mut self, chosen: &[usize]) {
        self.complete += 1;
        let placements = PlacementChoices {
            by_gear: self
                .plot
                .gears
                .iter()
                .zip(chosen)
                .enumerate()
                .map(|(depth, (gear, index))| {
                    (
                        gear.gear_id.clone(),
                        self.grids[depth][*index].choice.clone(),
                    )
                })
                .collect(),
        };
        let built = (|| {
            crate::requirements::validate_hard_requirements(
                self.plot,
                self.inputs.hosts,
                &placements,
                self.requirements,
            )?;
            let plan = crate::plan_with_options(
                self.plot,
                self.inputs.hosts,
                &placements,
                self.inputs.bases,
                self.inputs.options,
            )?;
            let plan = crate::characteristic_sealing::seal_characteristics(
                plan,
                self.inputs.advertisements,
            )?;
            observed_capacity(&plan, self.inputs.observations)?;
            Ok::<_, PlannerError>(plan)
        })();
        let Ok(plan) = built else {
            self.refused += 1;
            return;
        };
        let mut score = GlobalPlacementScore {
            unknown_operations: 0,
            unknown_transfers: 0,
            known_upper_seconds: ExactRational::ZERO,
            critical_path_upper_seconds: None,
        };
        for (depth, index) in chosen.iter().enumerate() {
            if let Some(forecast) = self.grids[depth][*index].forecast {
                let Ok(sum) = score
                    .known_upper_seconds
                    .checked_add(forecast.upper_seconds(), self.work)
                else {
                    self.disposition = GlobalPlacementDisposition::NumericLimit;
                    return;
                };
                score.known_upper_seconds = sum;
            } else {
                score.unknown_operations += 1;
            }
        }
        let transfers = if let Some(transfer) = self.transfer {
            // Edge estimates are incremental. Any operation model already
            // carrying transfer cannot be combined with them as known Time.
            let unscoped = chosen.iter().enumerate().any(|(depth, index)| {
                self.grids[depth][*index].forecast.is_some_and(|forecast| {
                    !forecast.forecast().stages().transfer.is_some_and(|range| {
                        range.lower().seconds() == ExactRational::ZERO
                            && range.upper().seconds() == ExactRational::ZERO
                    })
                })
            });
            if unscoped {
                self.refused += 1;
                return;
            }
            let Ok(edges) = transfer_objective::edge_costs(self.plot, &plan, transfer, self.inputs)
            else {
                self.refused += 1;
                return;
            };
            score.unknown_transfers = edges.iter().filter(|edge| edge.is_none()).count() as u16;
            Some(edges)
        } else {
            None
        };
        if let Some(order) = self.order {
            if score.unknown_operations == 0 && score.unknown_transfers == 0 {
                let costs: Vec<_> = chosen
                    .iter()
                    .enumerate()
                    .map(|(depth, index)| {
                        self.grids[depth][*index]
                            .forecast
                            .expect("known operands established")
                            .upper_seconds()
                    })
                    .collect();
                let edge_costs: Vec<_> = transfers
                    .as_ref()
                    .map(|edges| {
                        edges
                            .iter()
                            .map(|edge| edge.expect("known edge operands established"))
                            .collect()
                    })
                    .unwrap_or_else(
                        || alloc::vec![ExactRational::ZERO; self.plot.connections.len()],
                    );
                match objective::critical_path_with_transfers(
                    self.plot,
                    order,
                    &costs,
                    &edge_costs,
                    self.work,
                ) {
                    Ok(duration) => score.critical_path_upper_seconds = Some(duration),
                    Err(_) => {
                        self.disposition = GlobalPlacementDisposition::NumericLimit;
                        return;
                    }
                }
            }
        }
        let better = if let Some((_, previous)) = &self.best {
            match (score.unknown_operations + score.unknown_transfers)
                .cmp(&(previous.unknown_operations + previous.unknown_transfers))
            {
                Ordering::Less => true,
                Ordering::Greater => false,
                Ordering::Equal => {
                    let operands = match self.objective {
                        GlobalPlacementObjective::AggregateFulfillmentTime => {
                            Some((score.known_upper_seconds, previous.known_upper_seconds))
                        }
                        GlobalPlacementObjective::CriticalPathFulfillmentTime => score
                            .critical_path_upper_seconds
                            .zip(previous.critical_path_upper_seconds),
                    };
                    match operands {
                        None => false, // equally unknown path models retain stable enumeration
                        Some((left, right)) => match left.checked_compare(right, self.work) {
                            Ok(order) => order == Ordering::Less,
                            Err(_) => {
                                self.disposition = GlobalPlacementDisposition::NumericLimit;
                                return;
                            }
                        },
                    }
                }
            }
        } else {
            true
        };
        if better {
            self.best = Some((plan, score));
        }
    }
}
