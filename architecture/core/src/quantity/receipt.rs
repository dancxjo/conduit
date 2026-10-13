//! Checked literal evidence and immutable exact conversion facts.
use super::{ExactQuantityTargetCoordinate,Quantity,QuantityConversionRefusal,QuantityRefusal};
use crate::{Unit,QuantitySuffixRefusal,ResolvedQuantitySuffix};
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ExactQuantityConversionRequestRefusal {
 Source(QuantityRefusal),TemperatureDifferenceSource(super::ExactTemperatureDifferenceRefusal),
 Target(QuantitySuffixRefusal),TargetTooLong,SourceEvidenceMismatch,TargetEvidenceMismatch,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ExactQuantityConversionReceipt<'a> {
 original:&'a str,source:Quantity,source_suffix:ResolvedQuantitySuffix<'a>,target:ResolvedQuantitySuffix<'a>,
 result:Result<ExactQuantityTargetCoordinate<'a>,QuantityConversionRefusal>,
}
impl<'a> ExactQuantityConversionReceipt<'a> {
 pub fn from_checked(source:Quantity,target_unit:Unit,original:&'a str,target_source:&'a str)->Result<Self,ExactQuantityConversionRequestRefusal> {
  if target_source.len()>crate::UNIT_MAX_SOURCE_BYTES{return Err(ExactQuantityConversionRequestRefusal::TargetTooLong);}
  if !source.matches_literal_evidence(original){return Err(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch);}
  let source_suffix=source_suffix_from_evidence(original,&source)?;
  let target=ResolvedQuantitySuffix::from_unit(target_source,target_unit).map_err(|_|ExactQuantityConversionRequestRefusal::TargetEvidenceMismatch)?;
  let result=source.convert_to_target(target.clone());Ok(Self {original,source,source_suffix,target,result})
 }
 pub fn check(original:&'a str,target:&'a str)->Result<Self,ExactQuantityConversionRequestRefusal> {
  let source=Quantity::parse_plot_literal(original).map_err(ExactQuantityConversionRequestRefusal::Source)?;
  let target_unit=Unit::resolve(target).map_err(|e|ExactQuantityConversionRequestRefusal::Target(QuantitySuffixRefusal::Unit(e)))?;
  Self::from_checked(source,target_unit,original,target)
 }
 pub const fn original(&self)->&'a str {self.original}
 pub fn source(&self)->&Quantity {&self.source}
 pub fn source_suffix(&self)->&ResolvedQuantitySuffix<'a> {&self.source_suffix}
 pub fn target(&self)->&ResolvedQuantitySuffix<'a> {&self.target}
 pub fn result(&self)->Result<&ExactQuantityTargetCoordinate<'a>,QuantityConversionRefusal> {self.result.as_ref().map_err(|e|*e)}
}
pub(super) fn source_suffix_from_evidence<'a>(original:&'a str,source:&Quantity)->Result<ResolvedQuantitySuffix<'a>,ExactQuantityConversionRequestRefusal> {
 let unit=source.unit();let symbol=unit.symbol();
 let text=if original.trim_end().ends_with(')') {
  original.trim_end().strip_suffix(')').and_then(|body|body.rsplit_once(',')).map(|(_,s)|s.trim())
 }else {original.strip_suffix(symbol).map(|_|&original[original.len()-symbol.len()..])};
 ResolvedQuantitySuffix::from_unit(text.ok_or(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch)?,unit.to_owned()).map_err(|_|ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch)
}
