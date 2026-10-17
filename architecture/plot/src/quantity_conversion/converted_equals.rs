//! Explicit Boolean projection of a validated successful conversion receipt.
//! Refusal evidence remains in the original receipt. Invalid receipts fail closed.
mod validation;
use super::*;

pub const KIND: &str = "units/converted-equals";
pub const REVISION: &str = "quantity/converted-equals@1";

pub fn contract() -> Kind {
    let receipt = receipt_type()
        .profile()
        .expect("receipt")
        .value_kind()
        .clone();
    let expected = kind_id(QUANTITY_INFO_ID);
    Kind {
        kind_id: kind_id(KIND),
        kind_contract_revision: KindIdentity::from(REVISION),
        startup_parameters: vec![FrontStartupParameter {
            name: "expected".into(),
            value_type: expected.clone(),
            has_default: false,
        }],
        shorthand: Some((port_id("receipt"), port_id("result"))),
        inputs: vec![PortDescriptor {
            port_id: port_id("receipt"),
            value_kind: receipt.clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("result"),
            value_kind: kind_id(BOOL_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: vec![KindConfigurationField {
            key: "expected".into(),
            default_value: ConfigurationValue::Quantity(
                QuantityConfigurationValue::parse("0Hz").expect("default"),
            ),
            rule: KindConfigurationRule::Quantity,
        }],
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Input(port_id("receipt")),
            contract: CheckedValueContract::new(receipt, MAXIMUM_RECEIPT_BYTES, vec![])
                .expect("bounded receipt"),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_RECEIPT_BYTES,
        },
    }
}
pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    let contract = contract();
    startup.insert(KindSignature {
        kind: KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: "expected".into(),
            value_type: "Quantity".into(),
            default: None,
        }],
    })?;
    startup.insert_fore(KIND, contract.checked_front())?;
    profile.insert_kind(contract).map_err(|e| format!("{e}"))
}

/// Schema and expected value are admitted before Play. Comparison borrows the
/// canonical input and uses the existing bounded exact law without allocation.
pub struct PreparedConvertedEquals {
    expected: Quantity,
    prefix: Vec<u8>,
    validator: PreparedStructuredValueValidator,
}
impl PreparedConvertedEquals {
    pub fn new(expected: Quantity) -> Result<Self, QuantityConversionPreparationRefusal> {
        let ty = receipt_type();
        Ok(Self {
            expected,
            prefix: ty
                .canonical_bytes()
                .map_err(QuantityConversionPreparationRefusal::Receipt)?,
            validator: PreparedStructuredValueValidator::new(&ty, MAXIMUM_RECEIPT_BYTES as usize)
                .map_err(QuantityConversionPreparationRefusal::Receipt)?,
        })
    }
    pub fn compare_canonical_receipt(
        &self,
        input: &[u8],
    ) -> Result<bool, QuantityConversionPreparationRefusal> {
        use QuantityConversionPreparationRefusal as R;
        self.validator
            .validate(input)
            .map_err(|_| R::ForgedReceipt)?;
        let receipt = validate_canonical_structured_value(input).map_err(|_| R::ForgedReceipt)?;
        if receipt.type_bytes() != self.prefix {
            return Err(R::ForgedReceipt);
        }
        let (source, converted) = validation::validate(receipt)?;
        if !converted {
            return Ok(false);
        }
        match source.compare(self.expected) {
            Ok(core::cmp::Ordering::Equal) => Ok(true),
            Ok(_)
            | Err(
                QuantityConversionRefusal::IncompatibleDimensions
                | QuantityConversionRefusal::IncompatibleQuantityFamilies
                | QuantityConversionRefusal::IncompatibleQuantityRoles,
            ) => Ok(false),
            Err(_) => Err(R::Configuration),
        }
    }
}
pub fn compare_receipt(
    receipt: &StructuredInfoValue,
    expected: Quantity,
) -> Result<bool, QuantityConversionPreparationRefusal> {
    PreparedConvertedEquals::new(expected)?.compare_canonical_receipt(
        &receipt
            .canonical_bytes()
            .map_err(QuantityConversionPreparationRefusal::Receipt)?,
    )
}
