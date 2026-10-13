//! Exact target coordinates retain the full captured Unit law.
use super::{Quantity,QuantityConversionRefusal};
use crate::{MathValue,ResolvedQuantitySuffix};
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ExactQuantityTargetCoordinate<'a> {target:ResolvedQuantitySuffix<'a>,coordinate:MathValue}
impl<'a> ExactQuantityTargetCoordinate<'a> {
 pub fn target(&self)->&ResolvedQuantitySuffix<'a> {&self.target}
 pub fn coordinate(&self)->&MathValue {&self.coordinate}
}
impl Quantity {
 pub fn convert_to_target<'a>(&self,target:ResolvedQuantitySuffix<'a>)->Result<ExactQuantityTargetCoordinate<'a>,QuantityConversionRefusal> {
  let coordinate=self.convert_to_unit(&target.unit())?;Ok(ExactQuantityTargetCoordinate {target,coordinate})
 }
}
