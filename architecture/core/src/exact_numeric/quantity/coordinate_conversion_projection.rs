//! The existing physical law, retaining exact changes of wire representation.
use super::*;
use crate::projection::*;
use core::marker::PhantomData;
const VALUE: &str = "physical-denotation";
const ROLE: &str = "quantity-family-role";
const COORDINATES: &str = "unit-coordinates";
const WIRE: &str = "coordinate-wire-representation";
pub const EXACT_QUANTITY_COORDINATE_WIRE_LAW: &str = "numeric/exact-quantity-coordinate-wire@1";
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
#[derive(Debug, Clone, Copy, Default)]
pub struct QuantityCoordinateConversionDomain<'source, 'target>(
    PhantomData<(&'source [u8], &'target [u8])>,
);
impl QuantityCoordinateConversionDomain<'_, '_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
    pub fn facts(
        self,
        receipt: &QuantityCoordinateConversion<'_>,
    ) -> [ProjectionFact<'static, Self>; 4] {
        [
            ProjectionFact::Preserved {
                obligation: text(VALUE),
            },
            ProjectionFact::Preserved {
                obligation: text(ROLE),
            },
            ProjectionFact::Transformed {
                obligation: text(COORDINATES),
                law: text(
                    receipt
                        .reference_law()
                        .unwrap_or(MATHEMATICAL_QUANTITY_CONVERSION_LAW),
                ),
            },
            if receipt.source().representation() == receipt.target_representation() {
                ProjectionFact::Preserved {
                    obligation: text(WIRE),
                }
            } else {
                ProjectionFact::Transformed {
                    obligation: text(WIRE),
                    law: text(EXACT_QUANTITY_COORDINATE_WIRE_LAW),
                }
            },
        ]
    }
}
impl<'source, 'target> ProjectionDomain for QuantityCoordinateConversionDomain<'source, 'target> {
    type Source = QuantityCoordinateConversion<'source>;
    type Target = QuantityValueView<'target>;
    type Detail = ();
    fn source_contract(&self) -> ProjectionText<'_> {
        text(crate::QUANTITY_INFO_ID)
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        text(crate::QUANTITY_INFO_ID)
    }
    fn validate(
        &self,
        source: &Self::Source,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        if mechanism != ProjectionMechanism::Completed
            || !native.is_empty()
            || !scores.is_empty()
            || facts.len() != 4
        {
            return false;
        }
        let Some(target) = target else {
            return false;
        };
        if target.unit() != source.target_unit()
            || target.role() != source.source().role()
            || target.family() != source.source().family()
            || target.dimension() != source.source().dimension()
            || target.encoded().len() != source.encoded_bytes()
            || target.representation() != source.target_representation()
            || target.representation_digest() != source.target_representation_digest()
        {
            return false;
        }
        let wire = if source.source().representation() == source.target_representation() {
            facts.iter().any(|f| matches!(f, ProjectionFact::Preserved { obligation } if obligation.as_str()==WIRE))
        } else {
            facts.iter().any(|f| {
                matches!(f, ProjectionFact::Transformed { obligation,law }
            if obligation.as_str()==WIRE && law.as_str()==EXACT_QUANTITY_COORDINATE_WIRE_LAW)
            })
        };
        wire && facts.iter().any(|f| matches!(f, ProjectionFact::Preserved { obligation } if obligation.as_str()==VALUE))
            && facts.iter().any(|f| matches!(f, ProjectionFact::Preserved { obligation } if obligation.as_str()==ROLE))
            && facts.iter().any(|f| matches!(f, ProjectionFact::Transformed { obligation,law }
                if obligation.as_str()==COORDINATES && law.as_str()==source.reference_law().unwrap_or(MATHEMATICAL_QUANTITY_CONVERSION_LAW)))
    }
}
#[derive(Debug, Clone, Copy)]
pub struct QuantityCoordinateConversionPolicy {
    pub target_unit: Unit,
}
impl<'source, 'target> ProjectionPolicy<QuantityCoordinateConversionDomain<'source, 'target>>
    for QuantityCoordinateConversionPolicy
{
    fn identity(&self) -> ProjectionText<'_> {
        text("physical/exact-unit-coordinate-policy@1")
    }
    fn configuration_digest(&self) -> Option<[u8; 32]> {
        Some(crate::semantic_digest(
            "physical/exact-unit-coordinate-configuration@1",
            &self.target_unit.encode(),
        ))
    }
    fn permits(
        &self,
        source: &QuantityCoordinateConversion<'source>,
        target: &QuantityValueView<'target>,
        facts: &[ProjectionFact<'_, QuantityCoordinateConversionDomain<'source, 'target>>],
    ) -> bool {
        self.target_unit == source.target_unit()
            && QuantityCoordinateConversionDomain::new().validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
