//! Ordinary source-authored numerical requirements. The source schema retains
//! typed mathematical bounds; compact certificate custody uses the wire codec.
use super::*;
use crate::{
    kind_id, StructuredConfigurationValue, StructuredFieldType, StructuredFieldValue,
    StructuredInfoType as Type, StructuredInfoValue as Value, StructuredInfoValueShape as Shape,
    StructuredVariantCase,
};
use alloc::{vec, vec::Vec};

pub const MAX_SOURCE_NUMERIC_PARAMETERS: u16 = 32;
const BOUND_KIND: &str = "numeric/profile-7-128-16-16-2048@1";
pub(super) fn leaf(kind: &str) -> Type {
    Type::leaf(kind_id(kind)).expect("reviewed primitive")
}
pub(super) fn record(name: &str, fields: Vec<(&str, Type)>) -> Type {
    Type::record(
        kind_id(name),
        fields
            .into_iter()
            .map(|(name, ty)| StructuredFieldType::new(name, ty).expect("reviewed field"))
            .collect(),
    )
    .expect("finite reviewed record")
}
fn choice(name: &str, cases: Vec<(&str, Type)>) -> Type {
    Type::variant(
        kind_id(name),
        cases
            .into_iter()
            .map(|(tag, ty)| StructuredVariantCase::new(tag, ty).expect("reviewed case"))
            .collect(),
    )
    .expect("finite reviewed choice")
}
fn tags(name: &str, cases: &[&str]) -> Type {
    choice(
        name,
        cases
            .iter()
            .map(|tag| (*tag, leaf(crate::EMPTY_INFO_ID)))
            .collect(),
    )
}
pub(super) fn budget_type() -> Type {
    record(
        "numeric/execution-budget@1",
        vec![
            ("storageBytes", leaf("value/u32")),
            ("workSteps", leaf("value/u64")),
            ("limbs", leaf("value/u16")),
            ("symbolicNodes", leaf("value/u16")),
        ],
    )
}
fn exception_type() -> Type {
    record(
        "numeric/exception-policy@1",
        [
            "invalid",
            "divisionByZero",
            "overflow",
            "underflow",
            "inexact",
        ]
        .into_iter()
        .map(|key| (key, leaf(crate::BOOL_INFO_ID)))
        .collect(),
    )
}
fn parameter_type() -> Type {
    record(
        "numeric/parameter-range@1",
        vec![
            ("slot", leaf("value/u16")),
            ("lower", leaf(BOUND_KIND)),
            ("upper", leaf(BOUND_KIND)),
        ],
    )
}
fn accuracy_type() -> Type {
    choice(
        "numeric/accuracy@1",
        vec![
            ("exact", leaf(crate::EMPTY_INFO_ID)),
            ("absolute", leaf(BOUND_KIND)),
            (
                "relative",
                record(
                    "numeric/relative-accuracy@1",
                    vec![
                        ("relative", leaf(BOUND_KIND)),
                        ("zeroAbsolute", leaf(BOUND_KIND)),
                    ],
                ),
            ),
            ("ulps", leaf("value/u32")),
            ("enclosureWidth", leaf(BOUND_KIND)),
        ],
    )
}
fn semantics_type() -> Type {
    tags("numeric/semantics@1", &["mathematical", "ieeeOperational"])
}
fn representation_type() -> Type {
    tags(
        "numeric/representation@1",
        &[
            "checkedInteger",
            "rational",
            "symbolic",
            "interval",
            "binary64",
            "ieeeBinary64",
            "automatic",
        ],
    )
}
fn rounding_type() -> Type {
    tags(
        "numeric/rounding@1",
        &[
            "nearestEven",
            "nearestAway",
            "towardZero",
            "towardPositive",
            "towardNegative",
        ],
    )
}
pub fn numeric_requirement_info_type() -> Type {
    record(
        NUMERIC_REQUIREMENT_INFO_ID,
        vec![
            ("semantics", semantics_type()),
            ("representation", representation_type()),
            ("rounding", rounding_type()),
            ("accuracy", accuracy_type()),
            ("budget", budget_type()),
            ("exceptions", exception_type()),
            (
                "parameters",
                Type::sequence(parameter_type(), MAX_SOURCE_NUMERIC_PARAMETERS)
                    .expect("finite ranges"),
            ),
            ("reproducible", leaf(crate::BOOL_INFO_ID)),
        ],
    )
}
pub fn numeric_requirement_profile() -> crate::KindId {
    numeric_requirement_info_type()
        .profile()
        .expect("finite source requirement schema")
        .value_kind()
        .clone()
}
pub(super) fn invalid(_: crate::StructuredInfoRefusal) -> NumericRefusal {
    NumericRefusal::MalformedEncoding
}
pub(super) fn value_leaf(kind: &str, bytes: Vec<u8>) -> Result<Value, NumericRefusal> {
    Value::leaf(leaf(kind), bytes).map_err(invalid)
}
fn number(value: &MathValue) -> Result<Value, NumericRefusal> {
    value_leaf(BOUND_KIND, value.canonical_bytes()?)
}
fn empty() -> Result<Value, NumericRefusal> {
    value_leaf(crate::EMPTY_INFO_ID, Vec::new())
}
fn selected(ty: Type, tag: &str) -> Result<Value, NumericRefusal> {
    Value::variant(ty, tag, empty()?).map_err(invalid)
}
pub(super) fn value_record(ty: Type, fields: Vec<(&str, Value)>) -> Result<Value, NumericRefusal> {
    let fields = fields
        .into_iter()
        .map(|(key, value)| StructuredFieldValue::new(key, value).map_err(invalid))
        .collect::<Result<Vec<_>, _>>()?;
    Value::record(ty, fields).map_err(invalid)
}
pub(super) fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, NumericRefusal> {
    let Shape::Record(fields) = value.shape() else {
        return Err(NumericRefusal::MalformedEncoding);
    };
    fields
        .iter()
        .find(|field| field.name() == key)
        .map(|field| field.value())
        .ok_or(NumericRefusal::MalformedEncoding)
}
pub(super) fn bytes(value: &Value) -> Result<&[u8], NumericRefusal> {
    let Shape::Leaf(bytes) = value.shape() else {
        return Err(NumericRefusal::MalformedEncoding);
    };
    Ok(bytes)
}
pub(super) fn integer<const N: usize>(value: &Value) -> Result<[u8; N], NumericRefusal> {
    bytes(value)?
        .try_into()
        .map_err(|_| NumericRefusal::MalformedEncoding)
}
fn boolean(value: &Value) -> Result<bool, NumericRefusal> {
    crate::InfoBool::decode(bytes(value)?)
        .map(|value| value.get())
        .map_err(|_| NumericRefusal::MalformedEncoding)
}
fn variant(value: &Value) -> Result<(&str, &Value), NumericRefusal> {
    let Shape::Variant { tag, payload } = value.shape() else {
        return Err(NumericRefusal::MalformedEncoding);
    };
    Ok((tag, payload))
}
impl NumericRequirement {
    pub fn configuration(&self) -> Result<StructuredConfigurationValue, NumericRefusal> {
        self.validate()?;
        self.canonical_bytes()?;
        // The ordinary source profile is narrower than the maximal native wire
        // contract. Neither range count nor numeric leaf bounds are bypassed.
        if self.parameters.len() > usize::from(MAX_SOURCE_NUMERIC_PARAMETERS) {
            return Err(NumericRefusal::StorageBudget);
        }
        let semantics = selected(
            semantics_type(),
            match self.semantics {
                NumericSemantics::Mathematical => "mathematical",
                NumericSemantics::IeeeOperational => "ieeeOperational",
            },
        )?;
        let representation = selected(
            representation_type(),
            match self.representation {
                NumericRepresentation::CheckedI64 => "checkedInteger",
                NumericRepresentation::ExactRational => "rational",
                NumericRepresentation::Symbolic => "symbolic",
                NumericRepresentation::CertifiedInterval => "interval",
                NumericRepresentation::Binary64 => "binary64",
                NumericRepresentation::IeeeBinary64 => "ieeeBinary64",
                NumericRepresentation::Automatic => "automatic",
            },
        )?;
        let rounding = selected(
            rounding_type(),
            match self.rounding {
                Binary64Rounding::NearestEven => "nearestEven",
                Binary64Rounding::NearestAway => "nearestAway",
                Binary64Rounding::TowardZero => "towardZero",
                Binary64Rounding::TowardPositive => "towardPositive",
                Binary64Rounding::TowardNegative => "towardNegative",
            },
        )?;
        let (tag, payload) = match &self.accuracy {
            NumericAccuracy::Exact => ("exact", empty()?),
            NumericAccuracy::Absolute(value) => ("absolute", number(value)?),
            NumericAccuracy::Relative {
                relative,
                zero_absolute,
            } => (
                "relative",
                value_record(
                    record(
                        "numeric/relative-accuracy@1",
                        vec![
                            ("relative", leaf(BOUND_KIND)),
                            ("zeroAbsolute", leaf(BOUND_KIND)),
                        ],
                    ),
                    vec![
                        ("relative", number(relative)?),
                        ("zeroAbsolute", number(zero_absolute)?),
                    ],
                )?,
            ),
            NumericAccuracy::Ulps(value) => (
                "ulps",
                value_leaf("value/u32", value.to_le_bytes().to_vec())?,
            ),
            NumericAccuracy::EnclosureWidth(value) => ("enclosureWidth", number(value)?),
        };
        let accuracy = Value::variant(accuracy_type(), tag, payload).map_err(invalid)?;
        let budget = value_record(
            budget_type(),
            vec![
                (
                    "storageBytes",
                    value_leaf(
                        "value/u32",
                        self.budget.storage_bytes.to_le_bytes().to_vec(),
                    )?,
                ),
                (
                    "workSteps",
                    value_leaf("value/u64", self.budget.work_steps.to_le_bytes().to_vec())?,
                ),
                (
                    "limbs",
                    value_leaf("value/u16", self.budget.limbs.to_le_bytes().to_vec())?,
                ),
                (
                    "symbolicNodes",
                    value_leaf(
                        "value/u16",
                        self.budget.symbolic_nodes.to_le_bytes().to_vec(),
                    )?,
                ),
            ],
        )?;
        let policy = self.exceptions;
        let exceptions = value_record(
            exception_type(),
            [
                ("invalid", policy.invalid),
                ("divisionByZero", policy.division_by_zero),
                ("overflow", policy.overflow),
                ("underflow", policy.underflow),
                ("inexact", policy.inexact),
            ]
            .into_iter()
            .map(|(name, flag)| Ok((name, value_leaf(crate::BOOL_INFO_ID, vec![u8::from(flag)])?)))
            .collect::<Result<Vec<_>, NumericRefusal>>()?,
        )?;
        let parameters = self
            .parameters
            .iter()
            .map(|range| {
                value_record(
                    parameter_type(),
                    vec![
                        (
                            "slot",
                            value_leaf("value/u16", range.slot.to_le_bytes().to_vec())?,
                        ),
                        ("lower", number(&range.lower)?),
                        ("upper", number(&range.upper)?),
                    ],
                )
            })
            .collect::<Result<Vec<_>, NumericRefusal>>()?;
        let parameters = Value::sequence(
            Type::sequence(parameter_type(), MAX_SOURCE_NUMERIC_PARAMETERS).map_err(invalid)?,
            parameters,
        )
        .map_err(invalid)?;
        let value = value_record(
            numeric_requirement_info_type(),
            vec![
                ("semantics", semantics),
                ("representation", representation),
                ("rounding", rounding),
                ("accuracy", accuracy),
                ("budget", budget),
                ("exceptions", exceptions),
                ("parameters", parameters),
                (
                    "reproducible",
                    value_leaf(crate::BOOL_INFO_ID, vec![u8::from(self.reproducible)])?,
                ),
            ],
        )?;
        StructuredConfigurationValue::new(
            numeric_requirement_profile(),
            value.canonical_bytes().map_err(invalid)?,
        )
        .ok_or(NumericRefusal::MalformedEncoding)
    }
    pub fn from_configuration(
        config: &StructuredConfigurationValue,
    ) -> Result<Self, NumericRefusal> {
        if config.profile() != &numeric_requirement_profile() {
            return Err(NumericRefusal::MalformedEncoding);
        }
        let value = Value::from_canonical_bytes(config.canonical_value()).map_err(invalid)?;
        if value.value_type() != &numeric_requirement_info_type() {
            return Err(NumericRefusal::MalformedEncoding);
        }
        let semantics = match variant(field(&value, "semantics")?)?.0 {
            "mathematical" => NumericSemantics::Mathematical,
            "ieeeOperational" => NumericSemantics::IeeeOperational,
            _ => return Err(NumericRefusal::MalformedEncoding),
        };
        let representation = match variant(field(&value, "representation")?)?.0 {
            "checkedInteger" => NumericRepresentation::CheckedI64,
            "rational" => NumericRepresentation::ExactRational,
            "symbolic" => NumericRepresentation::Symbolic,
            "interval" => NumericRepresentation::CertifiedInterval,
            "binary64" => NumericRepresentation::Binary64,
            "ieeeBinary64" => NumericRepresentation::IeeeBinary64,
            "automatic" => NumericRepresentation::Automatic,
            _ => return Err(NumericRefusal::MalformedEncoding),
        };
        let rounding = match variant(field(&value, "rounding")?)?.0 {
            "nearestEven" => Binary64Rounding::NearestEven,
            "nearestAway" => Binary64Rounding::NearestAway,
            "towardZero" => Binary64Rounding::TowardZero,
            "towardPositive" => Binary64Rounding::TowardPositive,
            "towardNegative" => Binary64Rounding::TowardNegative,
            _ => return Err(NumericRefusal::MalformedEncoding),
        };
        let (tag, payload) = variant(field(&value, "accuracy")?)?;
        let accuracy = match tag {
            "exact" => NumericAccuracy::Exact,
            "absolute" => NumericAccuracy::Absolute(MathValue::decode(bytes(payload)?)?),
            "relative" => NumericAccuracy::Relative {
                relative: MathValue::decode(bytes(field(payload, "relative")?)?)?,
                zero_absolute: MathValue::decode(bytes(field(payload, "zeroAbsolute")?)?)?,
            },
            "ulps" => NumericAccuracy::Ulps(u32::from_le_bytes(integer(payload)?)),
            "enclosureWidth" => {
                NumericAccuracy::EnclosureWidth(MathValue::decode(bytes(payload)?)?)
            }
            _ => return Err(NumericRefusal::MalformedEncoding),
        };
        let source_budget = field(&value, "budget")?;
        let budget = NumericExecutionBudget {
            storage_bytes: u32::from_le_bytes(integer(field(source_budget, "storageBytes")?)?),
            work_steps: u64::from_le_bytes(integer(field(source_budget, "workSteps")?)?),
            limbs: u16::from_le_bytes(integer(field(source_budget, "limbs")?)?),
            symbolic_nodes: u16::from_le_bytes(integer(field(source_budget, "symbolicNodes")?)?),
        };
        let source_policy = field(&value, "exceptions")?;
        let exceptions = NumericExceptionPolicy {
            invalid: boolean(field(source_policy, "invalid")?)?,
            division_by_zero: boolean(field(source_policy, "divisionByZero")?)?,
            overflow: boolean(field(source_policy, "overflow")?)?,
            underflow: boolean(field(source_policy, "underflow")?)?,
            inexact: boolean(field(source_policy, "inexact")?)?,
        };
        let Shape::Collection(ranges) = field(&value, "parameters")?.shape() else {
            return Err(NumericRefusal::MalformedEncoding);
        };
        let parameters = ranges
            .iter()
            .map(|range| {
                Ok(NumericParameterRange {
                    slot: u16::from_le_bytes(integer(field(range, "slot")?)?),
                    lower: MathValue::decode(bytes(field(range, "lower")?)?)?,
                    upper: MathValue::decode(bytes(field(range, "upper")?)?)?,
                })
            })
            .collect::<Result<Vec<_>, NumericRefusal>>()?;
        let request = Self {
            semantics,
            representation,
            rounding,
            accuracy,
            budget,
            exceptions,
            parameters,
            reproducible: boolean(field(&value, "reproducible")?)?,
        };
        request.validate()?;
        request.canonical_bytes()?; // Also enforce the compact custody bound.
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> NumericRequirement {
        NumericRequirement {
            semantics: NumericSemantics::Mathematical,
            representation: NumericRepresentation::Binary64,
            rounding: Binary64Rounding::NearestEven,
            accuracy: NumericAccuracy::Absolute(MathValue::rational("1", "100").unwrap()),
            parameters: vec![NumericParameterRange {
                slot: 2,
                lower: MathValue::integer(-10),
                upper: MathValue::integer(10),
            }],
            budget: NumericExecutionBudget {
                storage_bytes: 4096,
                work_steps: 100_000,
                limbs: 128,
                symbolic_nodes: 128,
            },
            reproducible: true,
            exceptions: NumericExceptionPolicy {
                inexact: true,
                ..NumericExceptionPolicy::default()
            },
        }
    }
    #[test]
    fn typed_source_retains_rounding_range_and_operational_mode() {
        for rounding in [
            Binary64Rounding::NearestEven,
            Binary64Rounding::NearestAway,
            Binary64Rounding::TowardZero,
            Binary64Rounding::TowardPositive,
            Binary64Rounding::TowardNegative,
        ] {
            let mut request = request();
            request.rounding = rounding;
            for representation in [
                NumericRepresentation::CheckedI64,
                NumericRepresentation::ExactRational,
                NumericRepresentation::Symbolic,
                NumericRepresentation::CertifiedInterval,
                NumericRepresentation::Binary64,
                NumericRepresentation::IeeeBinary64,
                NumericRepresentation::Automatic,
            ] {
                request.representation = representation;
                request.semantics = if representation == NumericRepresentation::IeeeBinary64 {
                    NumericSemantics::IeeeOperational
                } else {
                    NumericSemantics::Mathematical
                };
                request.accuracy = if request.semantics == NumericSemantics::IeeeOperational {
                    NumericAccuracy::Exact
                } else {
                    NumericAccuracy::Absolute(MathValue::rational("1", "100").unwrap())
                };
                assert_eq!(
                    NumericRequirement::from_configuration(&request.configuration().unwrap())
                        .unwrap(),
                    request
                );
            }
        }
    }
    #[test]
    fn shape_valid_source_cannot_claim_a_negative_error_allowance() {
        let request = request();
        let source = request.configuration().unwrap();
        let value = Value::from_canonical_bytes(source.canonical_value()).unwrap();
        let Shape::Record(fields) = value.shape() else {
            panic!("source record");
        };
        let accuracy = Value::variant(
            accuracy_type(),
            "absolute",
            number(&MathValue::integer(-1)).unwrap(),
        )
        .unwrap();
        let forged = value_record(
            numeric_requirement_info_type(),
            fields
                .iter()
                .map(|field| {
                    (
                        field.name(),
                        if field.name() == "accuracy" {
                            accuracy.clone()
                        } else {
                            field.value().clone()
                        },
                    )
                })
                .collect(),
        )
        .unwrap();
        let forged = StructuredConfigurationValue::new(
            numeric_requirement_profile(),
            forged.canonical_bytes().unwrap(),
        )
        .unwrap();
        assert_eq!(
            NumericRequirement::from_configuration(&forged),
            Err(NumericRefusal::Domain)
        );
    }
    #[test]
    fn source_parameter_inventory_keeps_its_declared_finite_profile() {
        let mut request = request();
        request.parameters = (0..=MAX_SOURCE_NUMERIC_PARAMETERS)
            .map(|slot| NumericParameterRange {
                slot,
                lower: MathValue::integer(0),
                upper: MathValue::integer(1),
            })
            .collect();
        request.canonical_bytes().unwrap();
        assert_eq!(request.configuration(), Err(NumericRefusal::StorageBudget));
    }
}
