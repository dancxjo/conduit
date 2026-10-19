//! Exact Unit-coordinate evidence through the existing general projection law.
use super::{MathematicalQuantityCoordinateConversion, MathematicalQuantityView};
use crate::{projection::*, Unit};
use core::marker::PhantomData;
pub const MATHEMATICAL_QUANTITY_CONVERSION_LAW: &str = "physical/exact-affine-unit-coordinates@1";
const VALUE: &str = "physical-denotation";
const FAMILY_ROLE: &str = "quantity-family-role";
const COORDINATES: &str = "unit-coordinates";
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).expect("reviewed projection identity")
}
#[derive(Debug, Clone, Copy, Default)]
pub struct MathematicalQuantityConversionDomain<'source, 'target>(
    PhantomData<(&'source [u8], &'target [u8])>,
);
impl MathematicalQuantityConversionDomain<'_, '_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
    pub fn facts(self) -> [ProjectionFact<'static, Self>; 3] {
        [
            ProjectionFact::Preserved {
                obligation: text(VALUE),
            },
            ProjectionFact::Preserved {
                obligation: text(FAMILY_ROLE),
            },
            ProjectionFact::Transformed {
                obligation: text(COORDINATES),
                law: text(MATHEMATICAL_QUANTITY_CONVERSION_LAW),
            },
        ]
    }
}
impl<'source, 'target> ProjectionDomain for MathematicalQuantityConversionDomain<'source, 'target> {
    type Source = MathematicalQuantityCoordinateConversion<'source>;
    type Target = MathematicalQuantityView<'target>;
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
            || facts.len() != 3
        {
            return false;
        }
        let Some(target) = target else {
            return false;
        };
        // The private computational receipt was constructed by the bounded
        // physical conversion. Bind its output, not an arbitrary same-Type value.
        if target.unit() != source.target_unit()
            || target.role() != source.source().role()
            || target.family() != source.source().family()
            || target.dimension() != source.source().dimension()
            || target.encoded().len() != source.encoded_bytes()
            || target.representation_digest() != source.target_representation_digest()
        {
            return false;
        }
        facts.iter().any(|f|matches!(f,ProjectionFact::Preserved { obligation } if obligation.as_str()==VALUE))
            && facts.iter().any(|f|matches!(f,ProjectionFact::Preserved { obligation } if obligation.as_str()==FAMILY_ROLE))
            && facts.iter().any(|f|matches!(f,ProjectionFact::Transformed { obligation,law }
                if obligation.as_str()==COORDINATES && law.as_str()==MATHEMATICAL_QUANTITY_CONVERSION_LAW))
    }
}
#[derive(Debug, Clone, Copy)]
pub struct MathematicalQuantityConversionPolicy {
    pub target_unit: Unit,
}
impl<'source, 'target> ProjectionPolicy<MathematicalQuantityConversionDomain<'source, 'target>>
    for MathematicalQuantityConversionPolicy
{
    fn identity(&self) -> ProjectionText<'_> {
        text("physical/exact-unit-coordinate-policy@1")
    }
    fn permits(
        &self,
        source: &MathematicalQuantityCoordinateConversion<'source>,
        target: &MathematicalQuantityView<'target>,
        facts: &[ProjectionFact<'_, MathematicalQuantityConversionDomain<'source, 'target>>],
    ) -> bool {
        self.target_unit == source.target_unit()
            && MathematicalQuantityConversionDomain::new().validate(
                source,
                Some(target),
                facts,
                &[],
                &[],
                ProjectionMechanism::Completed,
            )
    }
}
