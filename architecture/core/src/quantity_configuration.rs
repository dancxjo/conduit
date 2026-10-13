//! Checked physical startup values with bounded, separately retained source evidence.

use crate::{Quantity, QuantityRefusal, Unit, UnitRefusal};
use alloc::{string::String, vec::Vec};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct QuantityConfigurationValue {
    canonical_value: Vec<u8>,
    source: String,
    computation:Option<QuantityComputationEvidence>,
}
#[derive(Debug,Clone,PartialEq,Eq,PartialOrd,Ord,Serialize,Deserialize)]
pub struct QuantityComputationEvidence {
    source_ast_identity:[u8;32],
    #[serde(deserialize_with="deserialize_graph_bytes")]
    graph:Vec<u8>,
    source_seal:[u8;32],
}
impl QuantityComputationEvidence {
    pub fn source_ast_identity(&self)->[u8;32] {self.source_ast_identity}
    pub fn graph(&self)->&[u8] {&self.graph}
    fn seal(source:&str,ast:[u8;32],graph:&[u8],quantity:&[u8])->[u8;32] {
        crate::semantic_digest_parts("source/quantity-computation@1",&[&(source.len() as u64).to_le_bytes(),source.as_bytes(),&ast,&(graph.len() as u64).to_le_bytes(),graph,&(quantity.len() as u64).to_le_bytes(),quantity])
    }
}

impl QuantityConfigurationValue {
    pub fn from_value(value: Quantity) -> Result<Self, QuantityRefusal> {
        let source = value.canonical_literal()?;
        Ok(Self {
            canonical_value: value.encode().to_vec(),
            source,
            computation:None,
        })
    }

    pub fn new(value: Quantity, source: String) -> Option<Self> {
        value.matches_literal_evidence(&source).then(|| Self {
            canonical_value: value.encode().to_vec(),
            source,
            computation:None,
        })
    }
    pub fn parse(source: &str) -> Result<Self, QuantityRefusal> {
        let value = Quantity::parse_plot_literal(source)?;
        Ok(Self {
            canonical_value: value.encode().to_vec(),
            source: source.into(),
            computation:None,
        })
    }
    pub fn value(&self) -> Quantity {
        Quantity::decode(&self.canonical_value).expect("checked exact quantity configuration")
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn canonical_value(&self) -> &[u8] {
        &self.canonical_value
    }
    pub fn computation(&self)->Option<&QuantityComputationEvidence> {self.computation.as_ref()}
    /// Plot checks the authored AST against its captured declaration context.
    /// Core independently rederives the closed graph's typed exact coordinate
    /// and freezes the supplied AST identity together with the authored bytes.
    pub fn from_checked_computation(value:Quantity,source:String,source_ast_identity:[u8;32],graph:&crate::NumericComputation)->Option<Self> {
        if source.len()>crate::MAX_NUMERIC_COMPUTATION_BYTES{return None;}
        let crate::NumericComputedResult::Number(result)=graph.evaluate_exact(&[]).ok()? else {return None;};
        let (family,role)=crate::parse_quantity_semantic_role(&result.semantic_kind)?;
        if family!=value.family().identity()||role!=value.role()||result.value.canonical_bytes().ok()?.as_slice()!=value.coordinate().encoded(){return None;}
        if result.unit.as_ref()?.encoded()!=value.unit().encoded(){return None;}
        let canonical_value=value.encode();let graph=graph.canonical_bytes().ok()?;
        let source_seal=QuantityComputationEvidence::seal(&source,source_ast_identity,&graph,&canonical_value);
        Some(Self {canonical_value,source,computation:Some(QuantityComputationEvidence {source_ast_identity,graph,source_seal})})
    }
}

impl<'de> Deserialize<'de> for QuantityConfigurationValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Encoded {
            #[serde(deserialize_with="deserialize_quantity_bytes") ]
            canonical_value: Vec<u8>,
            source: String,
            #[serde(default)]
            computation:Option<QuantityComputationEvidence>,
        }
        let encoded = Encoded::deserialize(deserializer)?;
        let value = Quantity::decode(&encoded.canonical_value)
            .map_err(|_| D::Error::custom("invalid exact quantity configuration encoding"))?;
        if let Some(proof)=encoded.computation {
            if QuantityComputationEvidence::seal(&encoded.source,proof.source_ast_identity,&proof.graph,&encoded.canonical_value)!=proof.source_seal {return Err(D::Error::custom("computed quantity source seal mismatch"));}
            let graph=crate::NumericComputation::decode_validate(&proof.graph).map_err(|_|D::Error::custom("invalid quantity computation"))?;
            Self::from_checked_computation(value,encoded.source,proof.source_ast_identity,&graph).ok_or_else(||D::Error::custom("computed quantity facts mismatch"))
        }else {
            Self::new(value, encoded.source).ok_or_else(|| D::Error::custom("exact quantity source evidence mismatch"))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct UnitConfigurationValue {
    canonical_value: Vec<u8>,
    source: String,
}

impl UnitConfigurationValue {
    pub fn new(value: Unit, source: String) -> Option<Self> {
        value.matches_source_evidence(&source).then(|| Self {
            canonical_value: value.encode().to_vec(),
            source,
        })
    }
    pub fn parse(source: &str) -> Result<Self, UnitRefusal> {
        let value = Unit::resolve(source)?;
        Ok(Self {
            canonical_value: value.encode().to_vec(),
            source: source.into(),
        })
    }
    pub fn value(&self) -> Unit {
        Unit::decode(&self.canonical_value).expect("checked unit configuration")
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn canonical_value(&self) -> &[u8] {
        &self.canonical_value
    }
}

impl<'de> Deserialize<'de> for UnitConfigurationValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Encoded {
            #[serde(deserialize_with="deserialize_unit_bytes") ]
            canonical_value: Vec<u8>,
            source: String,
        }
        let encoded = Encoded::deserialize(deserializer)?;
        let value = Unit::decode(&encoded.canonical_value)
            .map_err(|_| D::Error::custom("invalid unit configuration encoding"))?;
        Self::new(value, encoded.source)
            .ok_or_else(|| D::Error::custom("unit source evidence mismatch"))
    }
}

/// Intrinsic TemperatureDelta admission over the sole Quantity/provenance codec.
#[derive(Debug,Clone,PartialEq,Eq,PartialOrd,Ord,Serialize)]
#[serde(transparent)]
pub struct ExactTemperatureDifferenceConfigurationValue {quantity:QuantityConfigurationValue}
impl ExactTemperatureDifferenceConfigurationValue {
 pub fn new(value:crate::ExactTemperatureDifference,source:String)->Option<Self> {Some(Self {quantity:QuantityConfigurationValue::new(value.storage_coordinate(),source)?})}
 pub fn parse(source:&str)->Result<Self,crate::ExactTemperatureDifferenceRefusal> {
  let value=crate::ExactTemperatureDifference::parse_plot_literal(source)?;
  Self::new(value,source.into()).ok_or(crate::ExactTemperatureDifferenceRefusal::NotTemperature)
 }
 pub fn from_checked_computation(value:crate::ExactTemperatureDifference,source:String,source_ast_identity:[u8;32],graph:&crate::NumericComputation)->Option<Self> {
  Some(Self {quantity:QuantityConfigurationValue::from_checked_computation(value.storage_coordinate(),source,source_ast_identity,graph)?})
 }
 pub fn value(&self)->crate::ExactTemperatureDifference {crate::ExactTemperatureDifference::from_quantity(self.quantity.value()).expect("checked TemperatureDelta configuration")}
 pub fn source(&self)->&str {self.quantity.source()}
 pub fn canonical_value(&self)->&[u8] {self.quantity.canonical_value()}
 pub fn computation(&self)->Option<&QuantityComputationEvidence> {self.quantity.computation()}
 pub fn quantity_configuration(&self)->&QuantityConfigurationValue {&self.quantity}
}
impl<'de> Deserialize<'de> for ExactTemperatureDifferenceConfigurationValue {
 fn deserialize<D:Deserializer<'de>>(d:D)->Result<Self,D::Error> {
  let quantity=QuantityConfigurationValue::deserialize(d)?;
  crate::ExactTemperatureDifference::from_quantity(quantity.value()).map_err(|_|D::Error::custom("wrong TemperatureDelta family or role"))?;
  Ok(Self {quantity})
 }
}

fn deserialize_quantity_bytes<'de,D:Deserializer<'de>>(d:D)->Result<Vec<u8>,D::Error> {crate::physical_definition::deserialize_bounded::<D,{crate::MAX_QUANTITY_ENCODED_BYTES}>(d)}
fn deserialize_unit_bytes<'de,D:Deserializer<'de>>(d:D)->Result<Vec<u8>,D::Error> {crate::physical_definition::deserialize_bounded::<D,{crate::MAX_UNIT_ENCODED_BYTES}>(d)}
fn deserialize_graph_bytes<'de,D:Deserializer<'de>>(d:D)->Result<Vec<u8>,D::Error> {crate::physical_definition::deserialize_bounded::<D,{crate::MAX_NUMERIC_COMPUTATION_BYTES}>(d)}
