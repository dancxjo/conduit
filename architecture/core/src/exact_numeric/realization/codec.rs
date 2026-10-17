//! Canonical typed offer property. Decode recomputes its numerical certificate.
use super::*;
use crate::{kind_id, KindId, StructuredInfoType};
use alloc::vec::Vec;
mod enclosed;
mod float;
const INFO: &str = "numeric/prepared-realization@1";
fn ty() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(INFO)).expect("reviewed numeric property Type")
}
pub fn numerical_realization_profile() -> KindId {
    ty().profile()
        .expect("reviewed finite property profile")
        .value_kind()
        .clone()
}
fn rational(bytes: &mut Vec<u8>, q: ExactRational) -> Result<(), NumericalRealizationRefusal> {
    let mut buffer = [0; crate::EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let length = q.encode(&mut buffer)?;
    bytes.extend_from_slice(&(length as u16).to_le_bytes());
    bytes.extend_from_slice(&buffer[..length]);
    Ok(())
}
pub(super) fn encode(
    value: &PreparedNumericalRealization,
) -> Result<StructuredConfigurationValue, NumericalRealizationRefusal> {
    if let NumericalAlgorithm::EnclosedBinary64 { rounding } = value.algorithm {
        return enclosed::encode(value, rounding);
    }
    let mut bytes = Vec::new();
    bytes.push(1);
    bytes.push(match value.algorithm {
        NumericalAlgorithm::ExactRational => 0,
        NumericalAlgorithm::OutwardI64 => 1,
        NumericalAlgorithm::Binary64 { .. } => 2,
        NumericalAlgorithm::EnclosedBinary64 { .. } => unreachable!("version two certificate"),
    });
    if let NumericalAlgorithm::Binary64 { rounding } = value.algorithm {
        bytes.push(float::mode(rounding));
    }
    bytes.extend_from_slice(&value.admitted_limb_work.to_le_bytes());
    bytes.extend_from_slice(&value.spent_limb_work.to_le_bytes());
    rational(
        &mut bytes,
        value
            .source
            .as_rational()
            .ok_or(NumericalRealizationRefusal::UnsupportedDenotation)?,
    )?;
    rational(&mut bytes, value.prepared_error_limit)?;
    rational(&mut bytes, value.maximum_error)?;
    match &value.target {
        NumericalTarget::ExactRational(q) => rational(&mut bytes, **q)?,
        NumericalTarget::Binary64(algorithm) => float::encode(&mut bytes, algorithm)?,
        NumericalTarget::EnclosedBinary64(_) => unreachable!("version two certificate"),
        NumericalTarget::OutwardI64(interval) => {
            for endpoint in [interval.lower(), interval.upper()] {
                let (encoded, length) = endpoint.encode();
                bytes.extend_from_slice(&encoded[..length]);
            }
        }
    }
    property(bytes)
}
fn property(bytes: Vec<u8>) -> Result<StructuredConfigurationValue, NumericalRealizationRefusal> {
    let value = StructuredInfoValue::leaf(ty(), bytes)
        .map_err(|_| NumericalRealizationRefusal::InvalidCertificate)?;
    let canonical = value
        .canonical_bytes()
        .map_err(|_| NumericalRealizationRefusal::InvalidCertificate)?;
    if canonical.len() > crate::MAXIMUM_REALIZATION_PROPERTY_BYTES {
        return Err(NumericalRealizationRefusal::CertificateExtentExceeded);
    }
    StructuredConfigurationValue::new(numerical_realization_profile(), canonical)
        .ok_or(NumericalRealizationRefusal::InvalidCertificate)
}
struct Cursor<'a> {
    bytes: &'a [u8],
}
impl<'a> Cursor<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], NumericalRealizationRefusal> {
        let value = self
            .bytes
            .get(..length)
            .ok_or(NumericalRealizationRefusal::InvalidCertificate)?;
        self.bytes = &self.bytes[length..];
        Ok(value)
    }
    fn u64(&mut self) -> Result<u64, NumericalRealizationRefusal> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(
            |_| NumericalRealizationRefusal::InvalidCertificate,
        )?))
    }
    fn rational(
        &mut self,
        work: &mut ExactNumericWork,
    ) -> Result<ExactRational, NumericalRealizationRefusal> {
        let length = u16::from_le_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| NumericalRealizationRefusal::InvalidCertificate)?,
        );
        Ok(ExactRational::decode(
            self.take(usize::from(length))?,
            work,
        )?)
    }
}
pub(super) fn decode(
    property: &StructuredConfigurationValue,
    work: &mut ExactNumericWork,
) -> Result<PreparedNumericalRealization, NumericalRealizationRefusal> {
    use NumericalRealizationRefusal as R;
    if property.canonical_value().len() > crate::MAXIMUM_REALIZATION_PROPERTY_BYTES {
        return Err(R::CertificateExtentExceeded);
    }
    if *property.profile() != numerical_realization_profile() {
        return Err(R::InvalidCertificate);
    }
    let value = StructuredInfoValue::from_canonical_bytes(property.canonical_value())
        .map_err(|_| R::InvalidCertificate)?;
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(R::InvalidCertificate);
    };
    if bytes.first() == Some(&2) {
        return enclosed::decode(bytes, work);
    }
    let mut cursor = Cursor { bytes };
    if cursor.take(1)? != [1] {
        return Err(R::InvalidCertificate);
    }
    let algorithm = match cursor.take(1)? {
        [0] => NumericalAlgorithm::ExactRational,
        [1] => NumericalAlgorithm::OutwardI64,
        [2] => NumericalAlgorithm::Binary64 {
            rounding: float::read_mode(&mut cursor)?,
        },
        _ => return Err(R::InvalidCertificate),
    };
    let admitted = cursor.u64()?;
    let spent = cursor.u64()?;
    let source = MathematicalValue::rational(cursor.rational(work)?);
    let error_limit = cursor.rational(work)?;
    let expected_error = cursor.rational(work)?;
    // The verifier has its own caller-admitted work ceiling. A forged offer
    // cannot grant itself unlimited validation work or skip recomputation.
    let program = if matches!(algorithm, NumericalAlgorithm::Binary64 { .. }) {
        Some(float::program(&mut cursor, work)?)
    } else {
        None
    };
    let mut verification = ExactNumericWork::new(admitted.min(work.remaining()));
    let attempt = match (algorithm, program) {
        (NumericalAlgorithm::Binary64 { rounding }, Some((program, limits))) => {
            PreparedNumericalRealization::prepare_binary64(
                &source,
                &program,
                rounding,
                error_limit,
                limits,
                &mut verification,
            )
        }
        _ => PreparedNumericalRealization::prepare(
            &source,
            algorithm,
            error_limit,
            &mut verification,
        ),
    };
    work.charge(verification.spent())?;
    let mut prepared = attempt?;
    if prepared.spent_limb_work != spent
        || spent > admitted
        || prepared.maximum_error != expected_error
    {
        return Err(R::InvalidCertificate);
    }
    match prepared.target() {
        NumericalTarget::Binary64(algorithm) => {
            if !float::matches(&mut cursor, algorithm)? {
                return Err(R::InvalidCertificate);
            }
        }
        NumericalTarget::EnclosedBinary64(_) => return Err(R::InvalidCertificate),
        NumericalTarget::ExactRational(q) => {
            if cursor.rational(work)? != **q {
                return Err(R::InvalidCertificate);
            }
        }
        NumericalTarget::OutwardI64(interval) => {
            for endpoint in [interval.lower(), interval.upper()] {
                let (encoded, length) = endpoint.encode();
                if cursor.take(length)? != &encoded[..length] {
                    return Err(R::InvalidCertificate);
                }
            }
        }
    }
    if !cursor.bytes.is_empty() {
        return Err(R::InvalidCertificate);
    }
    prepared.admitted_limb_work = admitted;
    if let NumericalTarget::Binary64(algorithm) = &mut prepared.target {
        algorithm.restore_admitted_work(admitted);
    }
    Ok(prepared)
}
