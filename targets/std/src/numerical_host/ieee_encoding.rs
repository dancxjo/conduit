//! Prepared canonical IEEE result encoding; all heap storage precedes Play.
use conduit_core::*;
use conduit_semantic_catalog::{ieee_binary64_comparison_type, ieee_binary64_result_type};

pub struct PreparedIeeeEncoding {
    quiet: bool,
    booleans: [Vec<u8>; 2],
    value: PreparedStructuredComposer,
    flags: PreparedStructuredComposer,
    result: PreparedStructuredComposer,
}
const FLAG_NAMES: [&str; 5] = [
    "divide-by-zero",
    "inexact",
    "invalid",
    "overflow",
    "underflow",
];
fn field_type(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        unreachable!("reviewed IEEE record Type")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
impl PreparedIeeeEncoding {
    pub fn new(quiet: bool) -> Result<Self, StructuredInfoRefusal> {
        let result_type = if quiet {
            ieee_binary64_comparison_type()
        } else {
            ieee_binary64_result_type()
        };
        let flags_type = field_type(&result_type, "flags");
        let bool_type = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID))?;
        let boolean = |value: bool| {
            StructuredInfoValue::leaf(bool_type.clone(), InfoBool::new(value).encode().to_vec())
        };
        let booleans = [
            boolean(false)?.canonical_bytes()?,
            boolean(true)?.canonical_bytes()?,
        ];
        let flags = StructuredInfoValue::record(
            flags_type.clone(),
            FLAG_NAMES
                .iter()
                .map(|name| StructuredFieldValue::new(*name, boolean(false)?))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let value_type = field_type(&result_type, "value");
        let value = StructuredInfoValue::leaf(
            value_type.clone(),
            if quiet {
                InfoBool::FALSE.encode().to_vec()
            } else {
                0u64.to_le_bytes().to_vec()
            },
        )?;
        let result = StructuredInfoValue::record(
            result_type.clone(),
            vec![
                StructuredFieldValue::new("flags", flags.clone())?,
                StructuredFieldValue::new("value", value.clone())?,
            ],
        )?;
        let mut prepared = Self {
            quiet,
            booleans,
            value: PreparedStructuredComposer::new(&value_type, value.canonical_bytes()?.len())?,
            flags: PreparedStructuredComposer::new(&flags_type, flags.canonical_bytes()?.len())?,
            result: PreparedStructuredComposer::new(&result_type, result.canonical_bytes()?.len())?,
        };
        if quiet {
            prepared.comparison(false, IeeeBinary64ExceptionFlags::default())?;
        } else {
            prepared.arithmetic(IeeeBinary64Result {
                value: IeeeF64::from_bits(0),
                flags: IeeeBinary64ExceptionFlags::default(),
            })?;
        }
        Ok(prepared)
    }
    pub fn result_bytes(&self) -> &[u8] {
        self.result.encoded()
    }
    pub fn prepared_heap_bytes(&self) -> usize {
        self.booleans.iter().map(Vec::capacity).sum::<usize>()
            + self.value.prepared_heap_bytes()
            + self.flags.prepared_heap_bytes()
            + self.result.prepared_heap_bytes()
    }
    pub fn arithmetic(
        &mut self,
        result: IeeeBinary64Result,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        if self.quiet {
            return Err(StructuredInfoRefusal::WrongType);
        }
        self.encode(&result.value.bits().to_le_bytes(), result.flags)
    }
    pub fn comparison(
        &mut self,
        value: bool,
        flags: IeeeBinary64ExceptionFlags,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        if !self.quiet {
            return Err(StructuredInfoRefusal::WrongType);
        }
        self.encode(&InfoBool::new(value).encode(), flags)
    }
    fn encode(
        &mut self,
        value: &[u8],
        flags: IeeeBinary64ExceptionFlags,
    ) -> Result<&[u8], StructuredInfoRefusal> {
        let no = validate_canonical_structured_value(&self.booleans[0])?;
        let yes = validate_canonical_structured_value(&self.booleans[1])?;
        let flag = |value| if value { yes } else { no };
        let flags = self.flags.record(&[
            flag(flags.divide_by_zero),
            flag(flags.inexact),
            flag(flags.invalid),
            flag(flags.overflow),
            flag(flags.underflow),
        ])?;
        let value = self.value.leaf(value)?;
        self.result.record(&[
            validate_canonical_structured_value(flags)?,
            validate_canonical_structured_value(value)?,
        ])
    }
}
