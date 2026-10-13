//! TemperatureDelta is an intrinsic checked role of the sole Quantity carrier.
use super::{Quantity,QuantityRefusal,QuantityConversionRefusal,ExactQuantityConversionRequestRefusal,ExactQuantityConversionReceipt,ExactQuantityTargetCoordinate};
use crate::{Unit,MathValue,ResolvedQuantitySuffix};
pub const EXACT_TEMPERATURE_DIFFERENCE_INFO_ID:&str=crate::BUILTIN_TEMPERATURE_DELTA_INFO_ID;
pub fn exact_temperature_difference_type()->crate::StructuredInfoType {
 crate::StructuredInfoType::leaf(crate::kind_id(EXACT_TEMPERATURE_DIFFERENCE_INFO_ID)).expect("declared TemperatureDelta leaf")
}
#[derive(Debug,Copy,Clone,PartialEq,Eq)]
pub enum ExactTemperatureDifferenceRefusal {Coordinate(QuantityRefusal),NotTemperature}
#[derive(Debug,Clone,PartialEq,Eq,PartialOrd,Ord)]
pub struct ExactTemperatureDifference {coordinate:Quantity}
impl ExactTemperatureDifference {
 pub fn new(coefficient:i128,exponent:i16,unit:Unit)->Result<Self,ExactTemperatureDifferenceRefusal> {
  Self::from_quantity(Quantity::from_decimal_role(coefficient,exponent,unit,crate::QuantityRole::Delta).map_err(ExactTemperatureDifferenceRefusal::Coordinate)?)
 }
 pub fn from_math(value:MathValue,unit:&Unit)->Result<Self,ExactTemperatureDifferenceRefusal> {
  Self::from_quantity(Quantity::from_math_role(value,unit,crate::QuantityRole::Delta).map_err(|e|ExactTemperatureDifferenceRefusal::Coordinate(e.into()))?)
 }
 pub fn parse_plot_literal(source:&str)->Result<Self,ExactTemperatureDifferenceRefusal> {Self::from_quantity(Quantity::parse_plot_literal(source).map_err(ExactTemperatureDifferenceRefusal::Coordinate)?)}
 pub fn from_quantity(coordinate:Quantity)->Result<Self,ExactTemperatureDifferenceRefusal> {
  if coordinate.family().identity()!=crate::builtin_temperature_family().identity()||coordinate.role()!=crate::QuantityRole::Delta {return Err(ExactTemperatureDifferenceRefusal::NotTemperature);}
  Ok(Self {coordinate})
 }
 pub fn storage_coordinate(&self)->Quantity {self.coordinate.clone()}
 pub fn quantity(&self)->&Quantity {&self.coordinate}
 pub fn semantic_digest(&self)->[u8;32] {crate::semantic_digest(EXACT_TEMPERATURE_DIFFERENCE_INFO_ID,&self.coordinate.as_ref().identity())}
 pub fn compare(&self,other:&Self)->Result<Option<core::cmp::Ordering>,QuantityConversionRefusal> {self.coordinate.compare(&other.coordinate)}
 pub fn convert_to_unit(&self,target:&Unit)->Result<MathValue,QuantityConversionRefusal> {self.coordinate.convert_to_unit(target)}
 pub fn convert_to_target<'a>(&self,target:ResolvedQuantitySuffix<'a>)->Result<ExactTemperatureDifferenceTargetCoordinate<'a>,QuantityConversionRefusal> {self.coordinate.convert_to_target(target)}
}
pub type ExactTemperatureDifferenceTargetCoordinate<'a>=ExactQuantityTargetCoordinate<'a>;
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ExactTemperatureDifferenceConversionReceipt<'a> {source:ExactTemperatureDifference,receipt:ExactQuantityConversionReceipt<'a>}
impl<'a> ExactTemperatureDifferenceConversionReceipt<'a> {
 pub fn from_checked(source:ExactTemperatureDifference,target_unit:Unit,original:&'a str,target_source:&'a str)->Result<Self,ExactQuantityConversionRequestRefusal> {
  let receipt=ExactQuantityConversionReceipt::from_checked(source.storage_coordinate(),target_unit,original,target_source)?;
  Ok(Self {source,receipt})
 }
 pub fn check(original:&'a str,target:&'a str)->Result<Self,ExactQuantityConversionRequestRefusal> {
  let source=ExactTemperatureDifference::parse_plot_literal(original).map_err(ExactQuantityConversionRequestRefusal::TemperatureDifferenceSource)?;
  let unit=Unit::resolve(target).map_err(|e|ExactQuantityConversionRequestRefusal::Target(crate::QuantitySuffixRefusal::Unit(e)))?;
  Self::from_checked(source,unit,original,target)
 }
 pub fn source(&self)->&ExactTemperatureDifference {&self.source}
 pub const fn original(&self)->&'a str {self.receipt.original()}
 pub fn source_suffix(&self)->&ResolvedQuantitySuffix<'a> {self.receipt.source_suffix()}
 pub fn target(&self)->&ResolvedQuantitySuffix<'a> {self.receipt.target()}
 pub fn result(&self)->Result<&ExactTemperatureDifferenceTargetCoordinate<'a>,QuantityConversionRefusal> {self.receipt.result()}
}
