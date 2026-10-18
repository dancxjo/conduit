//! Source-specific offers bind existing installed evaluators before planning.
use crate::{kernel_preparation, normalize_capability_offers, StdHost};
use conduit_core::{CapabilityOffer, ConfigurationValue, OfferGeneration};
use conduit_plot::{ExpandedCanonicalPlot, PortableExpressionProgram};

const OWNED: &str = "std/source-expression/";
const MAXIMUM_OFFERS: usize = 32;

impl StdHost {
    /// Prepare at most 32 additional expression realizations from checked Source.
    /// The checked tape is offered where eligible; other supported programs use
    /// the existing reference Back. Selection never changes during Play.
    /// Custom offers survive replacement of this Host's previous source batch.
    /// Refusal leaves the advertisement, generation and resource ledger intact.
    pub fn prepare_expression_realizations(
        &mut self,
        plot: &ExpandedCanonicalPlot,
    ) -> Result<(), String> {
        plot.validate_expansion()
            .map_err(|why| format!("expression source identity: {why}"))?;
        if !plot
            .gears
            .iter()
            .any(|gear| expression_filter(gear).is_some())
            && !self.advertisement.capabilities.iter().any(owns)
        {
            return Ok(());
        }
        if !self.kernel_resources.is_idle() {
            return Err("expression offers cannot change active reservations".into());
        }
        let mut advertisement = self.advertisement.clone();
        advertisement.capabilities.retain(|offer| !owns(offer));
        let mut additional = 0;
        for gear in &plot.gears {
            let Some(filter) = expression_filter(gear) else {
                continue;
            };
            let [entry] = gear.configuration.as_slice() else {
                return Err("expression source requires one exact program".into());
            };
            let ("program", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value)
            else {
                return Err("expression source requires its canonical program".into());
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded)
                .map_err(|why| format!("expression source program: {why:?}"))?;
            let temporal = gear
                .inputs
                .first()
                .ok_or("expression input missing")?
                .temporal;
            let baseline = if filter {
                conduit_std_offers::pure_filter_std_offer(&program, temporal)
            } else {
                conduit_std_offers::pure_expression_std_offer(&program, temporal)
            }
            .map_err(|why| format!("expression source contract: {why:?}"))?;
            if !gear.accepts_realization(&baseline) {
                return Err("expression program differs from its checked Kind/Fore".into());
            }
            if advertisement
                .capabilities
                .iter()
                .any(|offer| gear.accepts_realization(offer))
            {
                continue;
            }
            if additional == MAXIMUM_OFFERS {
                return Err("std expression source preparation exceeds 32 offers".into());
            }
            let mut offer = if !filter {
                match conduit_std_offers::pure_scalar_tape_std_offer(&program, temporal) {
                    Ok(tape) => tape,
                    Err(why) => prepared_reference(&program, baseline, &why)?,
                }
            } else {
                prepared_reference(&program, baseline, "scalar-tape/filter-contract@1")?
            };
            offer.capability_id = format!("{OWNED}{}", offer.kind_id.as_str()).into();
            advertisement.capabilities.push(offer);
            additional += 1;
        }
        if advertisement.capabilities == self.advertisement.capabilities {
            return Ok(());
        }
        normalize_capability_offers(&mut advertisement.capabilities)?;
        if advertisement.capabilities == self.advertisement.capabilities {
            return Ok(());
        }
        advertisement.offer_generation = OfferGeneration(
            advertisement
                .offer_generation
                .0
                .checked_add(1)
                .ok_or("expression offer generation exhausted")?,
        );
        let ledger = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = ledger;
        Ok(())
    }
}

fn expression_filter(gear: &conduit_plot::CheckedGear) -> Option<bool> {
    let kind = gear.kind_id.as_str();
    if kind.starts_with("conduitese/pure-filter/") {
        Some(true)
    } else if kind.starts_with("conduitese/pure-expression/") {
        Some(false)
    } else {
        None
    }
}

fn owns(offer: &CapabilityOffer) -> bool {
    use conduit_std_offers::*;
    offer.capability_id.as_str().starts_with(OWNED)
        && matches!(
            (
                offer.implementation.implementation_id.as_str(),
                offer.implementation.execution_profile_id.as_str(),
                offer.implementation.artifact_id.as_str()
            ),
            (
                PURE_EXPRESSION_STD_IMPLEMENTATION,
                PURE_EXPRESSION_STD_PROFILE,
                PURE_EXPRESSION_STD_ARTIFACT
            ) | (
                PURE_EXPRESSION_STD_IMPLEMENTATION,
                PURE_EXACT_QUANTITY_STD_PROFILE,
                PURE_EXACT_QUANTITY_STD_ARTIFACT
            ) | (
                PURE_FILTER_STD_IMPLEMENTATION,
                PURE_FILTER_STD_PROFILE,
                PURE_FILTER_STD_ARTIFACT
            ) | (
                PURE_SCALAR_TAPE_STD_IMPLEMENTATION,
                PURE_SCALAR_TAPE_STD_PROFILE,
                PURE_SCALAR_TAPE_STD_ARTIFACT
            )
        )
}

fn prepared_reference(
    program: &PortableExpressionProgram,
    mut offer: CapabilityOffer,
    tape_refusal: &str,
) -> Result<CapabilityOffer, String> {
    // The reference format also lacks nominal-law and arithmetic proof
    // witnesses. Do not advertise it as a fallback that validates those laws.
    reference_validation::check(program)?;
    if let Some(exact) = (offer.kind_contract_revision.as_str()
        != conduit_plot::PURE_FILTER_REVISION)
        .then(|| {
            conduit_std_offers::pure_exact_quantity_std_offer(program, offer.inputs[0].temporal)
                .ok()
        })
        .flatten()
    {
        offer = exact;
    } else {
        conduit_plot::PreparedPortableExpressionEvaluator::new(program)
            .map_err(|why| format!("std source expression profile refused: {why:?}"))?;
    }
    offer
        .realization_properties
        .push(selection_evidence::property(tape_refusal)?);
    offer
        .realization_properties
        .sort_by(|a, b| a.profile().cmp(b.profile()));
    Ok(offer)
}

#[cfg(test)]
mod plan_evidence_tests;
mod reference_validation;
mod selection_evidence;
#[cfg(test)]
mod tests;
